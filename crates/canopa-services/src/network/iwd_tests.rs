use super::*;
use crate::testing::{TestBus, iwd::*};

fn network<'a>(overview: &'a Overview, name: &str) -> &'a Network {
    overview
        .networks
        .iter()
        .find(|network| network.name == name)
        .unwrap()
}

#[tokio::test]
async fn the_overview_has_devices_networks_in_range_and_saved_networks() {
    let bus = TestBus::new();
    let _iwd = FakeIwd::serve(&bus).await;
    let overview = read(&bus.connect().await).await.unwrap();
    assert_eq!(overview.devices.len(), 1);
    assert_eq!(overview.devices[0].name, "wlan0");
    assert_eq!(overview.devices[0].state.as_deref(), Some("connected"));
    let names: Vec<_> = overview.networks.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, ["Home", "Cafe", "Neighbour", "eduroam"]);
    let home = network(&overview, "Home");
    assert!(home.connected);
    assert_eq!(home.strength, 50);
    assert_eq!(
        home.known.as_deref(),
        Some(known_path("Home", "psk").as_str())
    );
    assert_eq!(
        network(&overview, "eduroam").security(),
        WifiSecurity::Enterprise
    );
    assert_eq!(network(&overview, "Cafe").known, None);
    let known: Vec<_> = overview
        .known
        .iter()
        .map(|k| (k.name.as_str(), k.autoconnect))
        .collect();
    assert_eq!(known, [("Home", true), ("Hotel", false)]);
    assert!(overview.in_range(&overview.known[0]).is_some());
    assert!(overview.in_range(&overview.known[1]).is_none());
}

#[tokio::test]
async fn each_change_reaches_iwd_and_a_new_passphrase_goes_through_the_agent() {
    let bus = TestBus::new();
    let iwd = FakeIwd::serve(&bus).await;
    let conn = bus.connect().await;
    let overview = read(&conn).await.unwrap();
    let device = &overview.devices[0];

    set_powered(&conn, device, false).await.unwrap();
    scan(&conn, device).await.unwrap();
    disconnect(&conn, network(&overview, "Home")).await.unwrap();
    assert_eq!(
        disconnect(&conn, network(&overview, "Home")).await,
        Err(Error::Changed)
    );
    connect(&conn, network(&overview, "Home"), None)
        .await
        .unwrap();
    connect(&conn, network(&overview, "Cafe"), None)
        .await
        .unwrap();
    assert_eq!(
        connect(&conn, network(&overview, "Neighbour"), Some("short")).await,
        Err(Error::Invalid("A Wi-Fi password has 8 to 63 characters."))
    );
    connect(&conn, network(&overview, "Neighbour"), Some("next door"))
        .await
        .unwrap();
    connect_hidden(&conn, device, "Attic", Some("up the stairs"))
        .await
        .unwrap();
    set_autoconnect(&conn, &overview.known[1], true)
        .await
        .unwrap();
    forget(&conn, &overview.known[0]).await.unwrap();
    assert_eq!(
        iwd.calls(),
        [
            "Powered false",
            "Scan",
            "Disconnect",
            "Connect Home",
            "Connect Cafe",
            "Connect Neighbour next door",
            "ConnectHidden Attic up the stairs",
            "AutoConnect Hotel true",
            "Forget Home",
        ]
    );
    // The agent lives for one attempt only.
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while iwd.state.lock().unwrap().agent.is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();

    let after = read(&conn).await.unwrap();
    let saved: Vec<_> = after.known.iter().map(|k| k.name.as_str()).collect();
    assert_eq!(saved, ["Attic", "Cafe", "Hotel", "Neighbour"]);
    // A path that no longer holds what was read is refused before acting.
    assert_eq!(forget(&conn, &overview.known[0]).await, Err(Error::Changed));
    let mut renamed = network(&overview, "Cafe").clone();
    renamed.name = "Other".into();
    assert_eq!(connect(&conn, &renamed, None).await, Err(Error::Changed));
}
