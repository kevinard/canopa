use super::*;
use std::sync::{Arc, Mutex};

pub(crate) fn player(name: &str, status: &str) -> Player {
    Player {
        id: PlayerId {
            bus: format!("{PREFIX}{name}"),
            owner: format!(":1.{name}"),
            server: "test".into(),
        },
        name: name.into(),
        title: "Track".into(),
        artist: "Artist".into(),
        track: "/track/one".into(),
        status: status.into(),
        can_play: true,
        can_pause: true,
        can_next: true,
        can_previous: false,
    }
}

#[test]
fn selection_prefers_playing_is_stable_and_clears_a_departed_pin() {
    let a = player("a", "Paused");
    let mut b = player("b", "Playing");
    let mut state = Snapshot::default();
    state.reconcile(vec![a.clone(), b.clone()]);
    assert_eq!(state.selected, Some(b.id.clone()));
    b.status = "Paused".into();
    state.reconcile(vec![a.clone(), b.clone()]);
    assert_eq!(
        state.selected,
        Some(b.id.clone()),
        "same-priority refresh does not change selection"
    );
    state.selected = Some(a.id.clone());
    state.pinned = true;
    b.status = "Playing".into();
    state.reconcile(vec![b.clone(), a.clone()]);
    assert_eq!(state.selected, Some(a.id));
    state.reconcile(vec![b.clone()]);
    assert!(!state.pinned);
    assert_eq!(state.selected, Some(b.id));
    state.reconcile(vec![]);
    assert!(state.current().is_none());
    assert!(state.available, "no players is different from no bus");
}

#[derive(Clone)]
struct FakePlayer(Arc<Mutex<Player>>, Arc<Mutex<Vec<String>>>);

#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl FakePlayer {
    #[zbus(property)]
    fn identity(&self) -> String {
        self.0.lock().unwrap().name.clone()
    }
}

struct FakePlayback(FakePlayer);

#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl FakePlayback {
    #[zbus(property)]
    fn playback_status(&self) -> String {
        self.0.0.lock().unwrap().status.clone()
    }
    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        let p = self.0.0.lock().unwrap();
        HashMap::from([
            (
                "mpris:trackid".into(),
                zbus::zvariant::Value::from(
                    zbus::zvariant::ObjectPath::try_from(p.track.as_str()).unwrap(),
                )
                .try_to_owned()
                .unwrap(),
            ),
            (
                "xesam:title".into(),
                zbus::zvariant::Value::from(p.title.as_str())
                    .try_to_owned()
                    .unwrap(),
            ),
            (
                "xesam:artist".into(),
                zbus::zvariant::Value::from(vec![p.artist.as_str()])
                    .try_to_owned()
                    .unwrap(),
            ),
        ])
    }
    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_play(&self) -> bool {
        self.0.0.lock().unwrap().can_play
    }
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        self.0.0.lock().unwrap().can_pause
    }
    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.0.0.lock().unwrap().can_next
    }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        self.0.0.lock().unwrap().can_previous
    }
    fn play(&self) {
        self.0.1.lock().unwrap().push("Play".into());
    }
    fn pause(&self) {
        self.0.1.lock().unwrap().push("Pause".into());
    }
    fn next(&self) {
        self.0.1.lock().unwrap().push("Next".into());
    }
    fn previous(&self) {
        self.0.1.lock().unwrap().push("Previous".into());
    }
}

async fn export(bus: &crate::test_bus::TestBus, player: Player) -> (zbus::Connection, FakePlayer) {
    let fake = FakePlayer(
        Arc::new(Mutex::new(player.clone())),
        Arc::new(Mutex::new(vec![])),
    );
    let conn = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .name(player.id.bus)
        .unwrap()
        .serve_at(MPRIS_PATH, fake.clone())
        .unwrap()
        .serve_at(MPRIS_PATH, FakePlayback(fake.clone()))
        .unwrap()
        .build()
        .await
        .unwrap();
    (conn, fake)
}

async fn changed(conn: &zbus::Connection) {
    conn.emit_signal(
        None::<&str>,
        MPRIS_PATH,
        "org.freedesktop.DBus.Properties",
        "PropertiesChanged",
        &(
            PLAYER,
            HashMap::<String, OwnedValue>::new(),
            vec!["Metadata", "PlaybackStatus"],
        ),
    )
    .await
    .unwrap();
}

async fn until(
    rx: &mut watch::Receiver<Snapshot>,
    predicate: impl Fn(&Snapshot) -> bool,
) -> Snapshot {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = rx.borrow_and_update().clone();
            if predicate(&snapshot) {
                return snapshot;
            }
            rx.changed().await.unwrap();
        }
    })
    .await
    .expect("media snapshot deadline")
}

#[tokio::test]
async fn live_discovery_metadata_controls_capabilities_and_replaced_owner() {
    let bus = crate::test_bus::TestBus::new();
    let (tx, mut rx) = watch::channel(Snapshot::default());
    let (commands, commands_rx) = mpsc::channel(8);
    let connection = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .name(SERVICE)
        .unwrap()
        .serve_at(
            PATH,
            MediaInterface {
                snapshots: rx.clone(),
                commands,
            },
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    let worker_connection = connection.clone();
    let worker = tokio::spawn(async move { connected(worker_connection, &tx, commands_rx).await });
    until(&mut rx, |s| s.available && s.players.is_empty()).await;

    let (first, fake) = export(&bus, player("test", "Playing")).await;
    let state = until(&mut rx, |s| s.current().is_some()).await;
    let captured = state.current().unwrap().clone();
    assert_eq!(captured.artist, "Artist");
    assert_eq!(captured.track, "/track/one");
    assert!(!captured.can_previous);
    let client_conn = bus.connect().await;
    let client = zbus::Proxy::new(&client_conn, SERVICE, PATH, INTERFACE)
        .await
        .unwrap();
    let request = |operation| Request::Control {
        player: captured.id.clone(),
        track: captured.track.clone(),
        operation,
    };
    client
        .call::<_, _, ()>(
            "Apply",
            &(serde_json::to_string(&request(Operation::Pause)).unwrap(),),
        )
        .await
        .unwrap();
    assert_eq!(*fake.1.lock().unwrap(), ["Pause"]);
    assert!(
        client
            .call::<_, _, ()>(
                "Apply",
                &(serde_json::to_string(&request(Operation::Previous)).unwrap(),)
            )
            .await
            .is_err()
    );
    assert_eq!(
        *fake.1.lock().unwrap(),
        ["Pause"],
        "unsupported control never reaches player"
    );

    let (other, _) = export(&bus, player("other", "Paused")).await;
    let multiple = until(&mut rx, |s| s.players.len() == 2).await;
    let other_id = multiple
        .players
        .iter()
        .find(|p| p.name == "other")
        .unwrap()
        .id
        .clone();
    client
        .call::<_, _, ()>(
            "Apply",
            &(serde_json::to_string(&Request::Select(Some(other_id.clone()))).unwrap(),),
        )
        .await
        .unwrap();
    until(&mut rx, |s| {
        s.pinned && s.selected.as_ref() == Some(&other_id)
    })
    .await;
    let json: String = client.call("Snapshot", &()).await.unwrap();
    let panel_state: Snapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(
        panel_state.selected,
        Some(other_id.clone()),
        "panel and bar share the manual choice"
    );
    client
        .call::<_, _, ()>(
            "Apply",
            &(serde_json::to_string(&Request::Select(None)).unwrap(),),
        )
        .await
        .unwrap();
    until(&mut rx, |s| {
        !s.pinned && s.selected.as_ref() == Some(&captured.id)
    })
    .await;
    other.release_name(other_id.bus.as_str()).await.unwrap();
    until(&mut rx, |s| s.players.len() == 1).await;

    {
        let mut p = fake.0.lock().unwrap();
        p.title = "New title".into();
        p.track = "/track/two".into();
        p.status = "Paused".into();
    }
    changed(&first).await;
    until(&mut rx, |s| {
        s.current()
            .is_some_and(|p| p.title == "New title" && p.status == "Paused")
    })
    .await;
    assert!(
        client
            .call::<_, _, ()>(
                "Apply",
                &(serde_json::to_string(&request(Operation::Next)).unwrap(),)
            )
            .await
            .is_err(),
        "old track click rejected"
    );

    first.release_name(captured.id.bus.as_str()).await.unwrap();
    until(&mut rx, |s| s.players.is_empty()).await;
    let (_second, replacement) = export(&bus, player("test", "Playing")).await;
    until(&mut rx, |s| {
        s.current().is_some_and(|p| p.id.owner != captured.id.owner)
    })
    .await;
    assert!(
        client
            .call::<_, _, ()>(
                "Apply",
                &(serde_json::to_string(&request(Operation::Next)).unwrap(),)
            )
            .await
            .is_err()
    );
    assert!(
        replacement.1.lock().unwrap().is_empty(),
        "late action never targets replacement"
    );

    worker.abort();
    let _ = worker.await;
}

#[test]
fn text_is_bounded_and_handles_missing_metadata() {
    assert_eq!(bounded("a\nb\tc"), "abc");
    assert_eq!(bounded(&"é".repeat(1000)).chars().count(), 512);
    let mut p = player("Radio", "Playing");
    p.title.clear();
    p.artist.clear();
    assert_eq!(p.label(), "Radio");
}
