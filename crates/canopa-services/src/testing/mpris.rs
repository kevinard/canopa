//! Fake MPRIS players.
use super::TestBus;
use crate::mpris::{MPRIS_PATH, PLAYER, PREFIX, Player, PlayerId};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};
use zbus::zvariant::OwnedValue;

/// A controllable player named `name` with track `/track/one`. Its owner and
/// server fields are placeholders until [`export`] reads them back.
pub fn player(name: &str, status: &str) -> Player {
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

/// A player whose properties come from the shared [`Player`]; method calls
/// are recorded by name in `.1`; `.2` injects slow or failing reads.
#[derive(Clone)]
pub struct FakePlayer(
    pub Arc<Mutex<Player>>,
    pub Arc<Mutex<Vec<String>>>,
    pub Arc<ReadBehavior>,
);

#[derive(Default)]
pub struct ReadBehavior {
    /// Delay each `Metadata` read.
    pub delay_ms: AtomicU64,
    /// `Metadata` reads so far.
    pub calls: AtomicUsize,
    pub fail_metadata: AtomicBool,
    /// Report an invalid playback state once.
    pub fail_state_once: AtomicBool,
    pub failures: AtomicUsize,
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
    fn stop(&self) {
        self.0.1.lock().unwrap().push("Stop".into());
    }
}

/// Export `player` under its bus name on a new connection.
pub async fn export(bus: &TestBus, player: Player) -> (zbus::Connection, FakePlayer) {
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

/// Emit a Metadata and PlaybackStatus change from `conn`.
pub async fn changed(conn: &zbus::Connection) {
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
