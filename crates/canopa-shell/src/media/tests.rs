use super::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

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
struct FakePlayer(
    Arc<Mutex<Player>>,
    Arc<Mutex<Vec<String>>>,
    Arc<ReadBehavior>,
);

#[derive(Default)]
struct ReadBehavior {
    delay_ms: AtomicU64,
    calls: AtomicUsize,
    fail_metadata: AtomicBool,
    fail_state_once: AtomicBool,
    failures: AtomicUsize,
}

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
        // GetAll may omit failed optional getters; make this one snapshot invalid
        // even if the server still returns a partial property dictionary.
        if self.0.2.fail_state_once.swap(false, Ordering::SeqCst) {
            return "Invalid".into();
        }
        self.0.0.lock().unwrap().status.clone()
    }
    #[zbus(property)]
    async fn metadata(&self) -> zbus::fdo::Result<HashMap<String, OwnedValue>> {
        self.0.2.calls.fetch_add(1, Ordering::SeqCst);
        let delay = self.0.2.delay_ms.load(Ordering::SeqCst);
        if delay > 0 {
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        if self.0.2.fail_metadata.load(Ordering::SeqCst) {
            self.0.2.failures.fetch_add(1, Ordering::SeqCst);
            return Err(zbus::fdo::Error::Failed(
                "Temporary metadata failure".into(),
            ));
        }
        let p = self.0.0.lock().unwrap();
        Ok(HashMap::from([
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
        ]))
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
        Arc::new(ReadBehavior::default()),
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

    let (other, other_fake) = export(&bus, player("other", "Paused")).await;
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
    // A metadata error must not masquerade as departure or lose a manual pin.
    other_fake.2.fail_metadata.store(true, Ordering::SeqCst);
    other_fake.2.fail_state_once.store(true, Ordering::SeqCst);
    changed(&other).await;
    tokio::time::timeout(Duration::from_secs(2), async {
        while other_fake.2.failures.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("metadata failure reached the worker");
    assert!(
        tokio::time::timeout(Duration::from_millis(150), rx.changed())
            .await
            .is_err(),
        "a failed player read changed the published selection"
    );
    other_fake.2.fail_metadata.store(false, Ordering::SeqCst);
    other_fake.0.lock().unwrap().title = "Recovered metadata".into();
    changed(&other).await;
    let recovered = until(&mut rx, |s| {
        s.players.iter().any(|p| p.title == "Recovered metadata")
    })
    .await;
    assert!(recovered.pinned);
    assert_eq!(recovered.selected, Some(other_id.clone()));
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

#[tokio::test]
async fn slow_player_refresh_does_not_block_other_player_controls_or_reread_every_player() {
    let bus = crate::test_bus::TestBus::new();
    let (good_conn, good) = export(&bus, player("zz_fast", "Playing")).await;
    let mut slow = Vec::new();
    for index in 0..18 {
        slow.push(export(&bus, player(&format!("slow_{index:02}"), "Paused")).await);
    }
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
    let worker = tokio::spawn(async move { connected(connection, &tx, commands_rx).await });
    let state = until(&mut rx, |state| state.players.len() == 19).await;
    let target = state.current().unwrap().clone();
    let client_conn = bus.connect().await;
    let client = zbus::Proxy::new(&client_conn, SERVICE, PATH, INTERFACE)
        .await
        .unwrap();
    for (_, fake) in &slow {
        fake.2.delay_ms.store(1500, Ordering::SeqCst);
    }
    let before = slow[0].1.2.calls.load(Ordering::SeqCst);
    changed(&slow[0].0).await;
    tokio::time::timeout(Duration::from_secs(1), async {
        while slow[0].1.2.calls.load(Ordering::SeqCst) == before {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("slow refresh started");
    let request = serde_json::to_string(&Request::Control {
        player: target.id,
        track: target.track,
        operation: Operation::Pause,
    })
    .unwrap();
    let result = tokio::time::timeout(
        Duration::from_millis(600),
        client.call::<_, _, ()>("Apply", &(request,)),
    )
    .await;
    assert!(
        matches!(result, Ok(Ok(()))),
        "an unrelated metadata refresh blocked the control deadline: {result:?}"
    );
    assert_eq!(*good.1.lock().unwrap(), ["Pause"]);
    good.0.lock().unwrap().title = "Updated fast player".into();
    changed(&good_conn).await;
    until(&mut rx, |state| {
        state
            .current()
            .is_some_and(|p| p.title == "Updated fast player")
    })
    .await;
    assert!(
        slow.iter()
            .skip(1)
            .all(|(_, fake)| fake.2.calls.load(Ordering::SeqCst) == 1),
        "one player's properties caused unrelated player metadata reads"
    );
    // A burst for the in-flight owner retains one refresh obligation, not 100 scans.
    for _ in 0..100 {
        changed(&slow[0].0).await;
    }
    slow[0].1.2.delay_ms.store(0, Ordering::SeqCst);
    slow[0].1.0.lock().unwrap().title = "Latest burst metadata".into();
    changed(&slow[0].0).await;
    until(&mut rx, |state| {
        state
            .players
            .iter()
            .any(|p| p.title == "Latest burst metadata")
    })
    .await;
    let reads = slow[0].1.2.calls.load(Ordering::SeqCst) - before;
    assert!(reads <= 3, "101 updates caused {reads} metadata reads");
    assert!(
        slow.iter()
            .skip(1)
            .all(|(_, fake)| fake.2.calls.load(Ordering::SeqCst) == 1)
    );
    eprintln!(
        "101 updates during a slow read: {reads} metadata reads; unrelated players unchanged"
    );
    // A target that itself stops replying fails within the command's deadline,
    // without sending the control after its revalidation timed out.
    good.2.delay_ms.store(1500, Ordering::SeqCst);
    let request = serde_json::to_string(&Request::Control {
        player: state.current().unwrap().id.clone(),
        track: state.current().unwrap().track.clone(),
        operation: Operation::Pause,
    })
    .unwrap();
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        client.call::<_, _, ()>("Apply", &(request,)),
    )
    .await
    .unwrap();
    assert!(result.is_err());
    assert_eq!(*good.1.lock().unwrap(), ["Pause"]);
    worker.abort();
    let _ = worker.await;
}

#[tokio::test]
async fn initial_equal_status_selection_uses_bus_order_despite_metadata_arrival_order() {
    for pin_fast_player in [false, true] {
        let bus = crate::test_bus::TestBus::new();
        let (_a, slow) = export(&bus, player("a", "Playing")).await;
        slow.2.delay_ms.store(300, Ordering::SeqCst);
        let (_b, _fast) = export(&bus, player("b", "Playing")).await;
        let (tx, mut rx) = watch::channel(Snapshot::default());
        let (commands, commands_rx) = mpsc::channel(8);
        let conn = bus.connect().await;
        let worker = tokio::spawn(async move { connected(conn, &tx, commands_rx).await });
        let early = until(&mut rx, |state| state.players.len() == 1).await;
        assert!(early.current().unwrap().id.bus.ends_with(".b"));
        if pin_fast_player {
            let (result, answer) = oneshot::channel();
            commands
                .send(Command {
                    expires: tokio::time::Instant::now() + Duration::from_secs(1),
                    request: Request::Select(early.selected),
                    result,
                })
                .await
                .unwrap();
            answer.await.unwrap().unwrap();
        }
        let complete = until(&mut rx, |state| state.players.len() == 2).await;
        assert_eq!(
            complete.current().unwrap().id.bus,
            format!("{PREFIX}{}", if pin_fast_player { "b" } else { "a" })
        );
        assert_eq!(complete.pinned, pin_fast_player);
        worker.abort();
        let _ = worker.await;
    }
}
