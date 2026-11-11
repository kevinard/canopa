//! A fake power-profiles-daemon under either of its names.
use super::TestBus;
use crate::power_profiles::SERVICES;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use zbus::zvariant::OwnedValue;

#[derive(Clone)]
struct State {
    active: Arc<Mutex<String>>,
    profiles: Vec<String>,
}

impl State {
    fn profiles(&self) -> Vec<HashMap<String, OwnedValue>> {
        self.profiles
            .iter()
            .map(|name| {
                HashMap::from([(
                    "Profile".to_string(),
                    OwnedValue::try_from(zbus::zvariant::Value::from(name.as_str())).unwrap(),
                )])
            })
            .collect()
    }
}

macro_rules! daemon {
    ($type:ident, $name:literal) => {
        struct $type(State);

        #[zbus::interface(name = $name)]
        impl $type {
            #[zbus(property)]
            fn active_profile(&self) -> String {
                self.0.active.lock().unwrap().clone()
            }
            #[zbus(property)]
            fn set_active_profile(&mut self, profile: String) {
                *self.0.active.lock().unwrap() = profile;
            }
            #[zbus(property)]
            fn profiles(&self) -> Vec<HashMap<String, OwnedValue>> {
                self.0.profiles()
            }
        }
    };
}

daemon!(UPowerProfiles, "org.freedesktop.UPower.PowerProfiles");
daemon!(HadessProfiles, "net.hadess.PowerProfiles");

/// power-profiles-daemon under `SERVICES[service]` (0: the UPower name,
/// 1: the `net.hadess` name). The first profile starts active.
pub struct FakePowerProfiles {
    /// Close it to simulate the daemon leaving.
    pub connection: zbus::Connection,
    active: Arc<Mutex<String>>,
}

impl FakePowerProfiles {
    pub async fn serve(bus: &TestBus, service: usize, profiles: &[&str]) -> Self {
        let connection = bus.connect().await;
        let state = State {
            active: Arc::new(Mutex::new(profiles[0].into())),
            profiles: profiles.iter().map(|name| name.to_string()).collect(),
        };
        let active = state.active.clone();
        let (name, path) = SERVICES[service];
        if service == 0 {
            connection
                .object_server()
                .at(path, UPowerProfiles(state))
                .await
                .unwrap();
        } else {
            connection
                .object_server()
                .at(path, HadessProfiles(state))
                .await
                .unwrap();
        }
        connection.request_name(name).await.unwrap();
        Self { connection, active }
    }

    /// The active profile as last set by a client.
    pub fn active(&self) -> String {
        self.active.lock().unwrap().clone()
    }
}
