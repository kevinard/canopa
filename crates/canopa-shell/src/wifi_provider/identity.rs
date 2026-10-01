//! Backend-issued handles bind a selection to daemon, device and network identity.
//! Object paths are routing information; a reused path never changes a handle's meaning.
use super::*;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
};
use zbus::zvariant::OwnedObjectPath;

pub(super) fn changed() -> WifiBackendError {
    WifiBackendError::new(WifiBackendErrorCategory::TargetChanged)
}

#[cfg(test)]
pub(super) async fn owner(
    conn: &zbus::Connection,
    service: &str,
) -> Result<String, WifiBackendError> {
    zbus::fdo::DBusProxy::new(conn)
        .await
        .map_err(|_| changed())?
        .get_name_owner(service.try_into().map_err(|_| changed())?)
        .await
        .map(|owner| owner.to_string())
        .map_err(|_| changed())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DeviceIdentity {
    pub service: &'static str,
    pub server: String,
    pub owner: String,
    pub path: OwnedObjectPath,
}

impl DeviceIdentity {
    pub fn new(
        conn: &zbus::Connection,
        service: &'static str,
        owner: &str,
        path: OwnedObjectPath,
    ) -> Self {
        Self {
            service,
            server: conn.server_guid().to_string(),
            owner: owner.into(),
            path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum NetworkIdentity {
    NetworkManager {
        ssid: Vec<u8>,
        flags: u32,
        wpa: u32,
        rsn: u32,
    },
    Iwd {
        name: String,
        kind: String,
    },
}

pub(super) struct Observation {
    pub identity: NetworkIdentity,
    pub label: String,
    pub signal_strength: u8,
    pub security: WifiSecurity,
    pub known: bool,
    pub active: bool,
}

#[derive(Default)]
pub(super) struct Targets {
    device: Option<(String, DeviceIdentity)>,
    networks: HashMap<String, NetworkIdentity>,
}

fn handle(kind: &str) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    // Handles are never reused by another backend incarnation in this process.
    let next = NEXT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .expect("Wi-Fi handle space exhausted");
    format!("{kind}:{next}")
}

impl Targets {
    pub fn publish(
        &mut self,
        device: DeviceIdentity,
        observations: Vec<Observation>,
        scanning: bool,
    ) -> WifiSnapshot {
        if self
            .device
            .as_ref()
            .is_none_or(|(_, previous)| *previous != device)
        {
            self.device = Some((handle("device"), device));
            self.networks.clear();
        }
        let mut previous: HashMap<_, _> = std::mem::take(&mut self.networks)
            .into_iter()
            .map(|(handle, identity)| (identity, handle))
            .collect();
        let networks = observations
            .into_iter()
            .map(|observation| {
                let id = previous
                    .entry(observation.identity.clone())
                    .or_insert_with(|| handle("network"))
                    .clone();
                self.networks.insert(id.clone(), observation.identity);
                WifiNetwork {
                    id,
                    label: observation.label,
                    signal_strength: observation.signal_strength,
                    security: observation.security,
                    known: observation.known,
                    active: observation.active,
                }
            })
            .collect();
        WifiSnapshot {
            device_id: self.device.as_ref().map(|(id, _)| id.clone()),
            networks,
            scanning,
        }
    }

    pub fn resolve(
        &self,
        request: &WifiConnectRequest,
    ) -> Result<(DeviceIdentity, Option<NetworkIdentity>), WifiBackendError> {
        let (_, device) = self
            .device
            .as_ref()
            .filter(|(id, _)| *id == request.device_id)
            .ok_or_else(changed)?;
        let network = if request.hidden {
            None
        } else {
            Some(
                self.networks
                    .get(request.network_id.as_deref().ok_or_else(changed)?)
                    .cloned()
                    .ok_or_else(changed)?,
            )
        };
        Ok((device.clone(), network))
    }

    pub fn clear(&mut self) {
        self.device = None;
        self.networks.clear();
    }
}
