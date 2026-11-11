//! power-profiles-daemon (also known as UPower's power profiles).
//!
//! The daemon has two names: `org.freedesktop.UPower.PowerProfiles` and the
//! older `net.hadess.PowerProfiles`. Clients use whichever answers first.
//! Only the three standard profiles are offered.
//!
//! ```no_run
//! use canopa_services::{ServiceState, power_profiles::{self, PowerProfiles}};
//! use futures_util::StreamExt;
//! # async fn page() -> Result<(), canopa_services::Error> {
//! let system = canopa_services::bus::system().await?;
//! let mut states = power_profiles::watch(system.clone());
//! while let Some(state) = states.next().await {
//!     let ServiceState::Available(profiles) = state else {
//!         println!("Power profiles are not available");
//!         continue;
//!     };
//!     println!("{} of {:?}", profiles.active, profiles.supported);
//!     if profiles.active != "power-saver" && profiles.supported.contains(&"power-saver") {
//!         profiles.set(&system, "power-saver").await?;
//!     }
//! }
//! # Ok(()) }
//! ```
use crate::{Error, ServiceState, bus::StateStream};
use std::{collections::HashMap, time::Duration};
use zbus::{Connection, Proxy, zvariant::OwnedValue};

/// The daemon's names and object paths, in preference order. Each name is
/// also its interface name.
pub const SERVICES: [(&str, &str); 2] = [
    (
        "org.freedesktop.UPower.PowerProfiles",
        "/org/freedesktop/UPower/PowerProfiles",
    ),
    ("net.hadess.PowerProfiles", "/net/hadess/PowerProfiles"),
];

/// The standard profiles, in display order.
pub const PROFILES: [&str; 3] = ["power-saver", "balanced", "performance"];

/// One reading: the active profile and which standard ones the hardware supports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PowerProfiles {
    service: usize,
    pub active: String,
    pub supported: Vec<&'static str>,
}

impl PowerProfiles {
    async fn proxy(connection: &Connection, service: usize) -> zbus::Result<Proxy<'_>> {
        let (name, path) = SERVICES[service];
        Proxy::new(connection, name, path, name).await
    }

    async fn read(connection: &Connection, service: usize) -> zbus::Result<Self> {
        let proxy = Self::proxy(connection, service).await?;
        let (active, profiles) = tokio::try_join!(
            proxy.get_property("ActiveProfile"),
            proxy.get_property::<Vec<HashMap<String, OwnedValue>>>("Profiles"),
        )?;
        let supported = PROFILES
            .into_iter()
            .filter(|name| {
                profiles.iter().any(|profile| {
                    profile
                        .get("Profile")
                        .and_then(|value| <&str>::try_from(value).ok())
                        == Some(*name)
                })
            })
            .collect();
        Ok(Self {
            service,
            active,
            supported,
        })
    }

    /// The first daemon name that answers within 800 ms, or `None`.
    pub async fn discover(connection: &Connection) -> Option<Self> {
        for service in 0..SERVICES.len() {
            if let Ok(Ok(profiles)) =
                tokio::time::timeout(Duration::from_millis(800), Self::read(connection, service))
                    .await
            {
                return Some(profiles);
            }
        }
        None
    }

    /// The bus name this reading came from.
    pub fn service(&self) -> &'static str {
        SERVICES[self.service].0
    }

    /// Activate `profile` after checking it is still supported.
    pub async fn set(&self, connection: &Connection, profile: &str) -> Result<(), Error> {
        let current = Self::read(connection, self.service).await?;
        if !current.supported.contains(&profile) {
            return Err(Error::Changed);
        }
        Self::proxy(connection, self.service)
            .await?
            .set_property("ActiveProfile", profile)
            .await
            .map_err(|error| Error::from(zbus::Error::from(error)))
    }
}

/// One-shot read of whichever daemon name answers.
pub async fn read(connection: &Connection) -> ServiceState<PowerProfiles> {
    PowerProfiles::discover(connection).await.into()
}

/// The profiles with recovery, re-read on each daemon signal (property
/// changes) and switching names when the daemon is restarted under the other.
pub fn watch(connection: Connection) -> StateStream<PowerProfiles> {
    crate::bus::recovering(move |tx| {
        let connection = connection.clone();
        tokio::spawn(async move {
            for (index, (service, _)) in SERVICES.into_iter().enumerate() {
                if !matches!(crate::bus::owner(&connection, service).await, Ok(Some(_))) {
                    continue;
                }
                let work = crate::bus::observe_signals(
                    connection.clone(),
                    service,
                    move |connection| async move { Ok(PowerProfiles::read(&connection, index).await?) },
                    tx.clone(),
                );
                crate::bus::while_service_owned(&connection, service, &tx, work).await;
                return;
            }
            let _ = tx.send(Err(()));
        });
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{TestBus, power_profiles::FakePowerProfiles};
    use futures_util::StreamExt;

    #[tokio::test]
    async fn profiles_are_read_set_and_watched_under_either_name() {
        let bus = TestBus::new();
        let client = bus.connect().await;
        assert_eq!(read(&client).await, ServiceState::Unavailable);
        let mut states = watch(client.clone());
        let next = async |states: &mut StateStream<PowerProfiles>| {
            tokio::time::timeout(Duration::from_secs(3), states.next())
                .await
                .unwrap()
                .unwrap()
        };
        assert_eq!(next(&mut states).await, ServiceState::Unavailable);
        let daemon =
            FakePowerProfiles::serve(&bus, 1, &["balanced", "power-saver", "custom"]).await;
        let ServiceState::Available(profiles) = next(&mut states).await else {
            panic!("profiles expected");
        };
        assert_eq!(profiles.service(), "net.hadess.PowerProfiles");
        assert_eq!(profiles.active, "balanced");
        assert_eq!(profiles.supported, ["power-saver", "balanced"]);
        assert_eq!(
            profiles.set(&client, "performance").await,
            Err(Error::Changed)
        );
        profiles.set(&client, "power-saver").await.unwrap();
        assert_eq!(daemon.active(), "power-saver");
        let ServiceState::Available(changed) = next(&mut states).await else {
            panic!("profiles expected");
        };
        assert_eq!(changed.active, "power-saver");
        daemon.connection.close().await.unwrap();
        assert_eq!(next(&mut states).await, ServiceState::Unavailable);
        let _daemon = FakePowerProfiles::serve(&bus, 0, &["balanced"]).await;
        let ServiceState::Available(profiles) = next(&mut states).await else {
            panic!("profiles expected");
        };
        assert_eq!(profiles.service(), "org.freedesktop.UPower.PowerProfiles");
    }
}
