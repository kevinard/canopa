//! GeoClue: this computer's approximate location, for an automatic time zone.
//!
//! GeoClue decides whether Canopa may know the location (its configuration
//! or an agent asks the person); a refusal is [`Error::NotAuthorized`]. Only
//! city-level accuracy is requested, once: [`locate`] starts a client, waits
//! for the first location and stops it.
//!
//! ```no_run
//! # async fn page() -> Result<(), canopa_services::Error> {
//! let system = canopa_services::bus::system().await?;
//! if canopa_services::geoclue::available(&system).await {
//!     let place = canopa_services::geoclue::locate(&system, "org.canopa.Settings").await?;
//!     println!("{:.1}, {:.1}", place.latitude, place.longitude);
//! }
//! # Ok(()) }
//! ```
use crate::{Error, bus::DEADLINE};
use futures_util::StreamExt;
use std::time::Duration;
use zbus::zvariant::OwnedObjectPath;

pub const SERVICE: &str = "org.freedesktop.GeoClue2";
pub const MANAGER_PATH: &str = "/org/freedesktop/GeoClue2/Manager";
/// GeoClue's city accuracy level.
const CITY: u32 = 4;
/// How long [`locate`] waits for a first location.
pub const LOCATE_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub latitude: f64,
    pub longitude: f64,
    /// Meters.
    pub accuracy: f64,
}

/// Whether GeoClue is running or can be started.
pub async fn available(connection: &zbus::Connection) -> bool {
    let Ok(dbus) = zbus::fdo::DBusProxy::new(connection).await else {
        return false;
    };
    let Ok(name) = zbus::names::BusName::try_from(SERVICE) else {
        return false;
    };
    if let Ok(Ok(true)) = tokio::time::timeout(DEADLINE, dbus.name_has_owner(name)).await {
        return true;
    }
    matches!(
        tokio::time::timeout(DEADLINE, dbus.list_activatable_names()).await,
        Ok(Ok(names)) if names.iter().any(|known| known.as_str() == SERVICE)
    )
}

async fn proxy(
    connection: &zbus::Connection,
    path: OwnedObjectPath,
    interface: &'static str,
) -> Result<zbus::Proxy<'static>, Error> {
    Ok(zbus::Proxy::new_owned(connection.clone(), SERVICE, path, interface).await?)
}

/// The location at city accuracy, once.
pub async fn locate(connection: &zbus::Connection, desktop_id: &str) -> Result<Place, Error> {
    let manager = proxy(
        connection,
        OwnedObjectPath::try_from(MANAGER_PATH).map_err(zbus::Error::from)?,
        "org.freedesktop.GeoClue2.Manager",
    )
    .await?;
    let path: OwnedObjectPath =
        tokio::time::timeout(DEADLINE, manager.call("GetClient", &())).await??;
    let client = proxy(connection, path, "org.freedesktop.GeoClue2.Client").await?;
    client.set_property("DesktopId", desktop_id).await?;
    client.set_property("RequestedAccuracyLevel", CITY).await?;
    let mut updates = client.receive_signal("LocationUpdated").await?;
    tokio::time::timeout(DEADLINE, client.call::<_, _, ()>("Start", &())).await??;
    let located = tokio::time::timeout(LOCATE_TIMEOUT, async {
        loop {
            let current: OwnedObjectPath = client.get_property("Location").await?;
            if current.as_str() != "/" {
                return Ok::<_, Error>(current);
            }
            let Some(signal) = updates.next().await else {
                return Err(Error::Unavailable);
            };
            let (_, new): (OwnedObjectPath, OwnedObjectPath) = signal.body().deserialize()?;
            if new.as_str() != "/" {
                return Ok(new);
            }
        }
    })
    .await;
    let _ = tokio::time::timeout(DEADLINE, client.call::<_, _, ()>("Stop", &())).await;
    let location = proxy(connection, located??, "org.freedesktop.GeoClue2.Location").await?;
    let latitude: f64 = location.get_property("Latitude").await?;
    let longitude: f64 = location.get_property("Longitude").await?;
    let accuracy: f64 = location.get_property("Accuracy").await.unwrap_or(0.0);
    if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
        return Err(Error::Refused(
            "GeoClue reported an impossible location".into(),
        ));
    }
    Ok(Place {
        latitude,
        longitude,
        accuracy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{TestBus, geoclue::FakeGeoclue};

    #[tokio::test]
    async fn locates_once_and_stops() {
        let bus = TestBus::new();
        let client = bus.connect().await;
        assert!(!available(&client).await);
        let fake = FakeGeoclue::serve(&bus).await;
        assert!(available(&client).await);
        let place = locate(&client, "org.canopa.Settings").await.unwrap();
        assert!((place.latitude - 48.85).abs() < 0.01);
        assert!((place.longitude - 2.35).abs() < 0.01);
        let state = fake.state.lock().unwrap();
        assert_eq!(state.desktop_id, "org.canopa.Settings");
        assert_eq!(state.accuracy, 4);
        assert!(!state.running, "stopped after the first location");
    }
}
