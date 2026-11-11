//! A fake GeoClue that places the computer in Paris.
use super::TestBus;
use crate::geoclue::{MANAGER_PATH, SERVICE};
use std::sync::{Arc, Mutex};
use zbus::{
    fdo,
    object_server::SignalEmitter,
    zvariant::{ObjectPath, OwnedObjectPath},
};

const CLIENT_PATH: &str = "/org/freedesktop/GeoClue2/Client/1";
const LOCATION_PATH: &str = "/org/freedesktop/GeoClue2/Client/1/Location/1";

/// What clients asked for.
#[derive(Default)]
pub struct GeoclueState {
    pub desktop_id: String,
    pub accuracy: u32,
    pub running: bool,
}

struct Manager;
struct Client(Arc<Mutex<GeoclueState>>);
struct Location;

#[zbus::interface(name = "org.freedesktop.GeoClue2.Manager")]
impl Manager {
    fn get_client(&self) -> OwnedObjectPath {
        OwnedObjectPath::try_from(CLIENT_PATH).unwrap()
    }
}

#[zbus::interface(name = "org.freedesktop.GeoClue2.Client")]
impl Client {
    async fn start(&self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) -> fdo::Result<()> {
        if self.0.lock().unwrap().desktop_id.is_empty() {
            return Err(fdo::Error::AccessDenied("no desktop id".into()));
        }
        self.0.lock().unwrap().running = true;
        Self::location_updated(
            &emitter,
            ObjectPath::try_from("/").unwrap(),
            ObjectPath::try_from(LOCATION_PATH).unwrap(),
        )
        .await?;
        Ok(())
    }
    fn stop(&self) {
        self.0.lock().unwrap().running = false;
    }
    #[zbus(signal)]
    async fn location_updated(
        emitter: &SignalEmitter<'_>,
        old: ObjectPath<'_>,
        new: ObjectPath<'_>,
    ) -> zbus::Result<()>;
    #[zbus(property)]
    fn location(&self) -> OwnedObjectPath {
        // Like GeoClue, nothing until the first update.
        OwnedObjectPath::try_from("/").unwrap()
    }
    #[zbus(property)]
    fn desktop_id(&self) -> String {
        self.0.lock().unwrap().desktop_id.clone()
    }
    #[zbus(property)]
    fn set_desktop_id(&self, id: String) {
        self.0.lock().unwrap().desktop_id = id;
    }
    #[zbus(property)]
    fn requested_accuracy_level(&self) -> u32 {
        self.0.lock().unwrap().accuracy
    }
    #[zbus(property)]
    fn set_requested_accuracy_level(&self, level: u32) {
        self.0.lock().unwrap().accuracy = level;
    }
}

#[zbus::interface(name = "org.freedesktop.GeoClue2.Location")]
impl Location {
    #[zbus(property)]
    fn latitude(&self) -> f64 {
        48.8566
    }
    #[zbus(property)]
    fn longitude(&self) -> f64 {
        2.3522
    }
    #[zbus(property)]
    fn accuracy(&self) -> f64 {
        5000.0
    }
}

/// GeoClue on a private bus.
pub struct FakeGeoclue {
    pub connection: zbus::Connection,
    pub state: Arc<Mutex<GeoclueState>>,
}

impl FakeGeoclue {
    pub async fn serve(bus: &TestBus) -> Self {
        let state = Arc::new(Mutex::new(GeoclueState::default()));
        let connection = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .serve_at(MANAGER_PATH, Manager)
            .unwrap()
            .serve_at(CLIENT_PATH, Client(state.clone()))
            .unwrap()
            .serve_at(LOCATION_PATH, Location)
            .unwrap()
            .name(SERVICE)
            .unwrap()
            .build()
            .await
            .unwrap();
        Self { connection, state }
    }
}
