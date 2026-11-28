mod runtime_backend;
use runtime_backend::verify_control;
pub use runtime_backend::{RuntimeBackend, choose_backend};
mod iwd_agent;
pub(crate) use iwd_agent::IwdAgentRegistration;
// D-Bus Wi-Fi adapters. Wire methods follow the NetworkManager and iwd APIs;
// all constructors accept an existing connection internally for isolated tests.
use super::identity::{DeviceIdentity, NetworkIdentity, Observation, Targets};
use super::*;
pub(crate) use crate::network::iwd::ManagedObjects;
use crate::network::iwd::{IwdNetworkProxy, IwdObjectManagerProxy, IwdStationProxy};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::Mutex;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

enum Adapter {
    NetworkManager(NetworkManagerBackend),
    Iwd(IwdBackend),
}

impl WifiControlBackend for Adapter {
    fn name(&self) -> BackendName {
        match self {
            Self::NetworkManager(_) => BackendName::NetworkManager,
            Self::Iwd(_) => BackendName::Iwd,
        }
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            can_scan_while_connecting: false,
        }
    }

    fn initial_snapshot<'a>(&'a mut self) -> WifiBackendFuture<'a, WifiSnapshot> {
        match self {
            Self::NetworkManager(backend) => Box::pin(async move { backend.snapshot().await }),
            Self::Iwd(backend) => Box::pin(async move { backend.snapshot().await }),
        }
    }

    fn scan<'a>(&'a mut self) -> WifiBackendFuture<'a, WifiSnapshot> {
        match self {
            Self::NetworkManager(backend) => Box::pin(async move { backend.scan().await }),
            Self::Iwd(backend) => Box::pin(async move { backend.scan().await }),
        }
    }

    fn connect<'a>(
        &'a mut self,
        request: WifiConnectRequest,
    ) -> WifiBackendFuture<'a, WifiSnapshot> {
        match self {
            Self::NetworkManager(backend) => {
                Box::pin(async move { backend.connect(request).await })
            }
            Self::Iwd(backend) => Box::pin(async move { backend.connect(request).await }),
        }
    }

    fn radio<'a>(&'a mut self) -> WifiBackendFuture<'a, Option<bool>> {
        match self {
            Self::NetworkManager(backend) => Box::pin(async move { backend.radio().await }),
            Self::Iwd(backend) => Box::pin(async move { backend.radio().await }),
        }
    }

    fn set_radio<'a>(&'a mut self, enabled: bool) -> WifiBackendFuture<'a, WifiSnapshot> {
        match self {
            Self::NetworkManager(backend) => {
                Box::pin(async move { backend.set_radio(enabled).await })
            }
            Self::Iwd(backend) => Box::pin(async move { backend.set_radio(enabled).await }),
        }
    }

    fn disconnect<'a>(&'a mut self, device_id: String) -> WifiBackendFuture<'a, WifiSnapshot> {
        match self {
            Self::NetworkManager(backend) => {
                Box::pin(async move { backend.disconnect(&device_id).await })
            }
            Self::Iwd(backend) => Box::pin(async move { backend.disconnect(&device_id).await }),
        }
    }

    fn forget<'a>(
        &'a mut self,
        request: WifiConnectRequest,
    ) -> WifiBackendFuture<'a, WifiSnapshot> {
        match self {
            Self::NetworkManager(backend) => Box::pin(async move { backend.forget(request).await }),
            Self::Iwd(backend) => Box::pin(async move { backend.forget(request).await }),
        }
    }
}

impl NetworkManagerBackend {
    async fn manager(&self) -> Result<zbus::Proxy<'_>, WifiBackendError> {
        zbus::proxy::Builder::new(&self.conn)
            .destination(self.owner.clone())
            .map_err(connection_error)?
            .path("/org/freedesktop/NetworkManager")
            .map_err(connection_error)?
            .interface("org.freedesktop.NetworkManager")
            .map_err(connection_error)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .map_err(connection_error)
    }

    async fn radio(&self) -> Result<Option<bool>, WifiBackendError> {
        self.verify_owner().await?;
        Ok(Some(
            self.manager()
                .await?
                .get_property::<bool>("WirelessEnabled")
                .await
                .map_err(connection_error)?,
        ))
    }

    async fn set_radio(&self, enabled: bool) -> Result<WifiSnapshot, WifiBackendError> {
        self.verify_owner().await?;
        self.manager()
            .await?
            .set_property("WirelessEnabled", enabled)
            .await
            .map_err(|error| connection_error(zbus::Error::from(error)))?;
        self.snapshot().await
    }

    async fn disconnect(&self, device_id: &str) -> Result<WifiSnapshot, WifiBackendError> {
        let device = self.targets.lock().unwrap().resolve_device(device_id)?;
        self.verify_owner().await?;
        zbus::proxy::Builder::<zbus::Proxy<'_>>::new(&self.conn)
            .destination(self.owner.clone())
            .map_err(connection_error)?
            .path(device.path)
            .map_err(connection_error)?
            .interface("org.freedesktop.NetworkManager.Device")
            .map_err(connection_error)?
            .build()
            .await
            .map_err(connection_error)?
            .call::<_, _, ()>("Disconnect", &())
            .await
            .map_err(connection_error)?;
        self.snapshot().await
    }

    async fn forget(&self, request: WifiConnectRequest) -> Result<WifiSnapshot, WifiBackendError> {
        let (device, network) = self.targets.lock().unwrap().resolve(&request)?;
        let Some(NetworkIdentity::NetworkManager {
            ssid,
            flags,
            wpa,
            rsn,
        }) = network
        else {
            return Err(identity::changed());
        };
        let security = nm_security(wpa | rsn, flags);
        self.verify_owner().await?;
        for (path, saved, key) in self.profiles(&device.path).await? {
            if saved == ssid && profile_security(&key) == security {
                NmSettingsConnectionProxy::builder(&self.conn)
                    .destination(self.owner.clone())
                    .map_err(connection_error)?
                    .path(path)
                    .map_err(connection_error)?
                    .build()
                    .await
                    .map_err(connection_error)?
                    .delete()
                    .await
                    .map_err(connection_error)?;
            }
        }
        self.snapshot().await
    }
}

impl IwdBackend {
    async fn adapter_path(&self) -> Result<Option<OwnedObjectPath>, WifiBackendError> {
        self.verify_owner().await?;
        let objects = IwdObjectManagerProxy::builder(&self.conn)
            .destination(self.owner.clone())
            .map_err(connection_error)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .map_err(connection_error)?
            .get_managed_objects()
            .await
            .map_err(connection_error)?;
        let mut devices: Vec<_> = objects
            .into_iter()
            .filter(|(_, interfaces)| interfaces.contains_key("net.connman.iwd.Device"))
            .map(|(path, _)| path)
            .collect();
        devices.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        Ok(devices.into_iter().next())
    }

    async fn device_proxy(
        &self,
        path: OwnedObjectPath,
        interface: &'static str,
    ) -> Result<zbus::Proxy<'_>, WifiBackendError> {
        zbus::proxy::Builder::new(&self.conn)
            .destination(self.owner.clone())
            .map_err(connection_error)?
            .path(path)
            .map_err(connection_error)?
            .interface(interface)
            .map_err(connection_error)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .map_err(connection_error)
    }

    async fn radio(&self) -> Result<Option<bool>, WifiBackendError> {
        let Some(path) = self.adapter_path().await? else {
            return Ok(None);
        };
        Ok(Some(
            self.device_proxy(path, "net.connman.iwd.Device")
                .await?
                .get_property::<bool>("Powered")
                .await
                .map_err(connection_error)?,
        ))
    }

    async fn set_radio(&self, enabled: bool) -> Result<WifiSnapshot, WifiBackendError> {
        let path = self
            .adapter_path()
            .await?
            .ok_or_else(|| WifiBackendError::new(WifiBackendErrorCategory::NoControl))?;
        self.device_proxy(path, "net.connman.iwd.Device")
            .await?
            .set_property("Powered", enabled)
            .await
            .map_err(|error| connection_error(zbus::Error::from(error)))?;
        self.snapshot().await
    }

    async fn disconnect(&self, device_id: &str) -> Result<WifiSnapshot, WifiBackendError> {
        let device = self.targets.lock().unwrap().resolve_device(device_id)?;
        self.device_proxy(device.path, "net.connman.iwd.Station")
            .await?
            .call::<_, _, ()>("Disconnect", &())
            .await
            .map_err(connection_error)?;
        self.snapshot().await
    }

    async fn forget(&self, request: WifiConnectRequest) -> Result<WifiSnapshot, WifiBackendError> {
        let (device, network) = self.targets.lock().unwrap().resolve(&request)?;
        let Some(NetworkIdentity::Iwd { name, kind }) = network else {
            return Err(identity::changed());
        };
        self.verify_owner().await?;
        let station = IwdStationProxy::builder(&self.conn)
            .destination(self.owner.clone())
            .map_err(connection_error)?
            .path(device.path.clone())
            .map_err(connection_error)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .map_err(connection_error)?;
        for (path, _) in station
            .get_ordered_networks()
            .await
            .map_err(connection_error)?
        {
            let network = IwdNetworkProxy::builder(&self.conn)
                .destination(self.owner.clone())
                .map_err(connection_error)?
                .path(path)
                .map_err(connection_error)?
                .cache_properties(zbus::proxy::CacheProperties::No)
                .build()
                .await
                .map_err(connection_error)?;
            if network.name().await.ok().as_ref() == Some(&name)
                && network.network_type().await.ok().as_ref() == Some(&kind)
            {
                let known = network.known_network().await.map_err(connection_error)?;
                if known.as_str() != "/" {
                    self.device_proxy(known, "net.connman.iwd.KnownNetwork")
                        .await?
                        .call::<_, _, ()>("Forget", &())
                        .await
                        .map_err(connection_error)?;
                }
            }
        }
        self.snapshot().await
    }
}

async fn nm_network_identity(
    proxy: &NmAccessPointProxy<'_>,
) -> Result<NetworkIdentity, WifiBackendError> {
    Ok(NetworkIdentity::NetworkManager {
        ssid: proxy.ssid().await.map_err(connection_error)?,
        flags: proxy.flags().await.map_err(connection_error)?,
        wpa: proxy.wpa_flags().await.map_err(connection_error)?,
        rsn: proxy.rsn_flags().await.map_err(connection_error)?,
    })
}

pub struct NetworkManagerBackend {
    owner: String,
    targets: Mutex<Targets>,
    cleanup: cleanup::Cleanup,
    conn: zbus::Connection,
}

impl NetworkManagerBackend {
    #[cfg(test)]
    async fn from_connection(conn: zbus::Connection) -> Result<Self, WifiBackendError> {
        let owner = identity::owner(&conn, "org.freedesktop.NetworkManager").await?;
        Ok(Self {
            conn,
            owner,
            targets: Mutex::new(Targets::default()),
            cleanup: cleanup::Cleanup::default(),
        })
    }

    async fn verify_owner(&self) -> Result<(), WifiBackendError> {
        verify_control(
            &self.conn,
            crate::network::bundle::Kind::NetworkManager,
            &self.owner,
        )
        .await
    }

    async fn device(&self) -> Result<OwnedObjectPath, WifiBackendError> {
        self.verify_owner().await?;
        let manager = NmManagerProxy::builder(&self.conn)
            .destination(self.owner.clone())
            .map_err(connection_error)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .map_err(scan_error)?;
        let mut candidates = Vec::new();
        for path in manager.get_devices().await.map_err(scan_error)? {
            let device = NmDeviceProxy::builder(&self.conn)
                .destination(self.owner.clone())
                .map_err(connection_error)?
                .path(path.clone())
                .map_err(scan_error)?
                .cache_properties(zbus::proxy::CacheProperties::No)
                .build()
                .await
                .map_err(scan_error)?;
            if device.device_type().await.map_err(scan_error)? == 2
                && device.managed().await.map_err(scan_error)?
            {
                candidates.push((device.state().await.unwrap_or(0) != 100, path));
            }
        }
        candidates.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.as_str().cmp(b.1.as_str())));
        candidates
            .into_iter()
            .next()
            .map(|(_, path)| path)
            .ok_or_else(|| WifiBackendError::new(WifiBackendErrorCategory::NoControl))
    }

    async fn profiles(
        &self,
        device: &OwnedObjectPath,
    ) -> Result<Vec<(OwnedObjectPath, Vec<u8>, String)>, WifiBackendError> {
        let device = NmDeviceProxy::builder(&self.conn)
            .destination(self.owner.clone())
            .map_err(connection_error)?
            .path(device.clone())
            .map_err(scan_error)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .map_err(scan_error)?;
        let mut profiles = Vec::new();
        for path in device.available_connections().await.map_err(scan_error)? {
            let proxy = NmSettingsConnectionProxy::builder(&self.conn)
                .destination(self.owner.clone())
                .map_err(connection_error)?
                .path(path.clone())
                .map_err(scan_error)?
                .cache_properties(zbus::proxy::CacheProperties::No)
                .build()
                .await
                .map_err(scan_error)?;
            // Profiles without permission to read are not usable known networks.
            let Ok(mut settings) = proxy.get_settings().await else {
                continue;
            };
            let Some(mut wireless) = settings.remove("802-11-wireless") else {
                continue;
            };
            let Some(ssid) = wireless
                .remove("ssid")
                .and_then(|value| Vec::<u8>::try_from(value).ok())
            else {
                continue;
            };
            let key = settings
                .get("802-11-wireless-security")
                .and_then(|s| s.get("key-mgmt"))
                .and_then(|v| <&str>::try_from(v).ok())
                .unwrap_or("")
                .to_string();
            profiles.push((path, ssid, key));
        }
        Ok(profiles)
    }

    async fn snapshot(&self) -> Result<WifiSnapshot, WifiBackendError> {
        let device_path = self.device().await?;
        let profiles = self.profiles(&device_path).await?;
        let device = NmDeviceProxy::builder(&self.conn)
            .destination(self.owner.clone())
            .map_err(connection_error)?
            .path(device_path.clone())
            .map_err(scan_error)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .map_err(scan_error)?;
        let connected = device.state().await.unwrap_or(0) == 100;
        let wireless = NmWirelessDeviceProxy::builder(&self.conn)
            .destination(self.owner.clone())
            .map_err(connection_error)?
            .path(device_path.clone())
            .map_err(scan_error)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .map_err(scan_error)?;
        let active_access_point = wireless
            .active_access_point()
            .await
            .unwrap_or_else(|_| root_object_path());
        let mut networks = Vec::new();
        for path in wireless.access_points().await.map_err(scan_error)? {
            let proxy = NmAccessPointProxy::builder(&self.conn)
                .destination(self.owner.clone())
                .map_err(connection_error)?
                .path(path.clone())
                .map_err(scan_error)?
                .cache_properties(zbus::proxy::CacheProperties::No)
                .build()
                .await
                .map_err(scan_error)?;
            let Ok(fingerprint) = nm_network_identity(&proxy).await else {
                continue;
            };
            let NetworkIdentity::NetworkManager {
                ssid,
                flags,
                wpa,
                rsn,
            } = &fingerprint
            else {
                unreachable!()
            };
            if ssid.is_empty() {
                continue;
            }
            let security = nm_security(wpa | rsn, *flags);
            let label = String::from_utf8_lossy(ssid).into_owned();
            let known = profiles
                .iter()
                .any(|(_, saved, key)| saved == ssid && profile_security(key) == security);
            networks.push(Observation {
                identity: fingerprint,
                label,
                signal_strength: proxy.strength().await.unwrap_or(0),
                security,
                known,
                active: connected && path == active_access_point,
            });
        }
        self.verify_owner().await?;
        Ok(self.targets.lock().unwrap().publish(
            DeviceIdentity::new(
                &self.conn,
                "org.freedesktop.NetworkManager",
                &self.owner,
                device_path,
            ),
            networks,
            false,
        ))
    }

    async fn scan(&self) -> Result<WifiSnapshot, WifiBackendError> {
        let path = self.device().await?;
        let wireless = NmWirelessDeviceProxy::builder(&self.conn)
            .destination(self.owner.clone())
            .map_err(connection_error)?
            .path(path)
            .map_err(scan_error)?
            .build()
            .await
            .map_err(scan_error)?;
        let mut changes = wireless.receive_last_scan_changed().await;
        let before = wireless.last_scan().await.map_err(scan_error)?;
        wireless
            .request_scan(HashMap::new())
            .await
            .map_err(scan_error)?;
        tokio::time::timeout(std::time::Duration::from_secs(15), async {
            loop {
                if wireless.last_scan().await.map_err(scan_error)? > before {
                    return Ok(());
                }
                changes
                    .next()
                    .await
                    .ok_or_else(|| WifiBackendError::new(WifiBackendErrorCategory::ScanFailed))?;
            }
        })
        .await
        .map_err(|_| WifiBackendError::new(WifiBackendErrorCategory::ScanFailed))??;
        self.snapshot().await
    }

    async fn connect(&self, request: WifiConnectRequest) -> Result<WifiSnapshot, WifiBackendError> {
        validate_credential(&request)?;
        let (device_identity, network) = self.targets.lock().unwrap().resolve(&request)?;
        self.verify_owner().await?;
        let device = device_identity.path;
        let manager = NmManagerProxy::builder(&self.conn)
            .destination(self.owner.clone())
            .map_err(connection_error)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .map_err(connection_error)?;
        if !manager
            .get_devices()
            .await
            .map_err(connection_error)?
            .contains(&device)
        {
            return Err(identity::changed());
        }
        let selected_device = NmDeviceProxy::builder(&self.conn)
            .destination(self.owner.clone())
            .map_err(connection_error)?
            .path(device.clone())
            .map_err(connection_error)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .map_err(connection_error)?;
        if selected_device
            .device_type()
            .await
            .map_err(connection_error)?
            != 2
            || !selected_device.managed().await.map_err(connection_error)?
        {
            return Err(identity::changed());
        }
        let (ap, ssid, security, key_mgmt) = if request.hidden {
            (
                root_object_path(),
                request.label.as_bytes().to_vec(),
                WifiSecurity::Personal,
                "wpa-psk",
            )
        } else {
            let expected = network.as_ref().ok_or_else(identity::changed)?;
            let NetworkIdentity::NetworkManager {
                ssid,
                flags,
                wpa,
                rsn,
            } = expected
            else {
                return Err(identity::changed());
            };
            let wireless = NmWirelessDeviceProxy::builder(&self.conn)
                .destination(self.owner.clone())
                .map_err(connection_error)?
                .path(device.clone())
                .map_err(connection_error)?
                .cache_properties(zbus::proxy::CacheProperties::No)
                .build()
                .await
                .map_err(connection_error)?;
            let mut matched = None;
            for path in wireless.access_points().await.map_err(connection_error)? {
                let proxy = NmAccessPointProxy::builder(&self.conn)
                    .destination(self.owner.clone())
                    .map_err(connection_error)?
                    .path(path.clone())
                    .map_err(connection_error)?
                    .cache_properties(zbus::proxy::CacheProperties::No)
                    .build()
                    .await
                    .map_err(connection_error)?;
                if nm_network_identity(&proxy).await.ok().as_ref() == Some(expected) {
                    matched = Some(path);
                    break;
                }
            }
            let flags_combined = wpa | rsn;
            (
                matched.ok_or_else(identity::changed)?,
                ssid.clone(),
                nm_security(flags_combined, *flags),
                if flags_combined & 0x100 == 0 && flags_combined & 0x400 != 0 {
                    "sae"
                } else {
                    "wpa-psk"
                },
            )
        };
        let saved = self
            .profiles(&device)
            .await?
            .into_iter()
            .find(|(_, saved, key)| *saved == ssid && profile_security(key) == security);
        self.verify_owner().await?;
        if let Some(expected) = network.as_ref() {
            let proxy = NmAccessPointProxy::builder(&self.conn)
                .destination(self.owner.clone())
                .map_err(connection_error)?
                .path(ap.clone())
                .map_err(connection_error)?
                .cache_properties(zbus::proxy::CacheProperties::No)
                .build()
                .await
                .map_err(connection_error)?;
            if nm_network_identity(&proxy).await.ok().as_ref() != Some(expected) {
                return Err(identity::changed());
            }
        }
        let mut created = None;
        let active = if let Some((profile, _, _)) = saved.filter(|_| request.credential.is_none()) {
            manager
                .activate_connection(&profile, &device, &ap)
                .await
                .map_err(connection_error)?
        } else {
            if security == WifiSecurity::Enterprise {
                return Err(WifiBackendError::new(
                    WifiBackendErrorCategory::UnsupportedNetwork,
                ));
            }
            if security == WifiSecurity::Personal && request.credential.is_none() {
                return Err(WifiBackendError::new(
                    WifiBackendErrorCategory::CredentialRejected,
                ));
            }
            let settings = nm_settings(&request, ssid, security, key_mgmt);
            let conn = self.conn.clone();
            let owner = self.owner.clone();
            let cleanup = self.cleanup.clone();
            let (tx, rx) = tokio::sync::oneshot::channel();
            // Let the request finish even if its UI future is canceled: the
            // resulting guard must own the daemon-side resource before cleanup.
            self.cleanup.spawn(async move {
                let result = tokio::time::timeout(std::time::Duration::from_secs(30), async {
                    let manager = NmManagerProxy::builder(&conn)
                        .destination(owner.clone())
                        .map_err(connection_error)?
                        .cache_properties(zbus::proxy::CacheProperties::No)
                        .build()
                        .await
                        .map_err(connection_error)?;
                    verify_control(&conn, crate::network::bundle::Kind::NetworkManager, &owner)
                        .await?;
                    let options = HashMap::from([("persist".to_string(), Value::from("memory"))]);
                    let (profile, active, _) = manager
                        .add_and_activate_connection2(settings, &device, &ap, options)
                        .await
                        .map_err(connection_error)?;
                    Ok::<_, WifiBackendError>(NmPendingProfile {
                        conn: conn.clone(),
                        owner: owner.clone(),
                        cleanup,
                        resource: Some((profile, active)),
                    })
                })
                .await
                .unwrap_or_else(|_| {
                    Err(WifiBackendError::new(
                        WifiBackendErrorCategory::ConnectionFailed,
                    ))
                });
                let _ = tx.send(result);
            });
            let pending = rx
                .await
                .map_err(|_| WifiBackendError::new(WifiBackendErrorCategory::ConnectionFailed))??;
            let active = pending.resource.as_ref().unwrap().1.clone();
            created = Some(pending);
            active
        };
        let result = self.wait_for_activation(active).await;
        if result.is_ok()
            && let Some(pending) = &mut created
        {
            let proxy = NmSettingsConnectionProxy::builder(&self.conn)
                .destination(self.owner.clone())
                .map_err(connection_error)?
                .path(pending.resource.as_ref().unwrap().0.clone())
                .map_err(connection_error)?
                .cache_properties(zbus::proxy::CacheProperties::No)
                .build()
                .await
                .map_err(connection_error)?;
            self.verify_owner().await?;
            proxy.save().await.map_err(connection_error)?;
            pending.resource = None;
        }
        result?;
        self.snapshot().await
    }

    async fn wait_for_activation(&self, path: OwnedObjectPath) -> Result<(), WifiBackendError> {
        let proxy = NmActiveProxy::builder(&self.conn)
            .destination(self.owner.clone())
            .map_err(connection_error)?
            .path(path)
            .map_err(connection_error)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .map_err(connection_error)?;
        let mut changes = proxy
            .receive_activation_changed()
            .await
            .map_err(connection_error)?;
        tokio::time::timeout(std::time::Duration::from_secs(60), async {
            let state = proxy.state().await.map_err(connection_error)?;
            if state == 2 {
                return Ok(());
            }
            if state >= 3 {
                return Err(WifiBackendError::new(
                    WifiBackendErrorCategory::ConnectionFailed,
                ));
            }
            while let Some(signal) = changes.next().await {
                let args = signal.args().map_err(connection_error)?;
                if args.state == 2 {
                    return Ok(());
                }
                if args.state >= 3 {
                    return Err(WifiBackendError::new(if matches!(args.reason, 9 | 10) {
                        WifiBackendErrorCategory::CredentialRejected
                    } else {
                        WifiBackendErrorCategory::ConnectionFailed
                    }));
                }
            }
            Err(WifiBackendError::new(
                WifiBackendErrorCategory::ConnectionFailed,
            ))
        })
        .await
        .map_err(|_| WifiBackendError::new(WifiBackendErrorCategory::ConnectionFailed))?
    }
}

struct NmPendingProfile {
    owner: String,
    conn: zbus::Connection,
    cleanup: cleanup::Cleanup,
    resource: Option<(OwnedObjectPath, OwnedObjectPath)>,
}
impl Drop for NmPendingProfile {
    fn drop(&mut self) {
        let Some((profile, active)) = self.resource.take() else {
            return;
        };
        let conn = self.conn.clone();
        let owner = self.owner.clone();
        self.cleanup.spawn(async move {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                if let Ok(manager) = NmManagerProxy::builder(&conn)
                    .destination(owner.clone())
                    .unwrap()
                    .cache_properties(zbus::proxy::CacheProperties::No)
                    .build()
                    .await
                {
                    let _ = manager.deactivate_connection(&active).await;
                }
                if let Ok(proxy) = NmSettingsConnectionProxy::builder(&conn)
                    .destination(owner.clone())
                    .unwrap()
                    .path(profile)
                    .unwrap()
                    .cache_properties(zbus::proxy::CacheProperties::No)
                    .build()
                    .await
                {
                    let _ = proxy.delete().await;
                }
            })
            .await;
        });
    }
}

fn nm_security(flags: u32, ap_flags: u32) -> WifiSecurity {
    if flags & (0x200 | 0x2000) != 0 {
        WifiSecurity::Enterprise
    } else if flags & (0x100 | 0x400) != 0 {
        WifiSecurity::Personal
    } else if ap_flags & 1 != 0 {
        WifiSecurity::Enterprise
    }
    // WEP/unsupported security must not be treated as open.
    else {
        WifiSecurity::Open
    }
}

fn profile_security(key: &str) -> WifiSecurity {
    match key {
        "" => WifiSecurity::Open,
        "wpa-psk" | "sae" => WifiSecurity::Personal,
        _ => WifiSecurity::Enterprise,
    }
}

pub(crate) type Settings = HashMap<String, HashMap<String, OwnedValue>>;
fn owned(value: impl Into<Value<'static>>) -> OwnedValue {
    value
        .into()
        .try_into()
        .expect("owned scalar/array has no file descriptors")
}
pub(crate) fn nm_settings(
    request: &WifiConnectRequest,
    ssid: Vec<u8>,
    security: WifiSecurity,
    key: &str,
) -> Settings {
    let mut settings = HashMap::from([
        (
            "connection".into(),
            HashMap::from([
                ("id".into(), owned(request.label.clone())),
                ("type".into(), owned("802-11-wireless")),
                ("autoconnect".into(), owned(true)),
            ]),
        ),
        (
            "802-11-wireless".into(),
            HashMap::from([
                ("ssid".into(), owned(ssid)),
                ("hidden".into(), owned(request.hidden)),
            ]),
        ),
        (
            "ipv4".into(),
            HashMap::from([("method".into(), owned("auto"))]),
        ),
        (
            "ipv6".into(),
            HashMap::from([("method".into(), owned("auto"))]),
        ),
    ]);
    if security == WifiSecurity::Personal {
        settings.insert(
            "802-11-wireless-security".into(),
            HashMap::from([
                ("key-mgmt".into(), owned(key.to_string())),
                (
                    "psk".into(),
                    owned(request.credential.clone().unwrap_or_default()),
                ),
            ]),
        );
    }
    settings
}

pub struct IwdBackend {
    owner: String,
    targets: Mutex<Targets>,
    cleanup: cleanup::Cleanup,
    conn: zbus::Connection,
}

impl IwdBackend {
    #[cfg(test)]
    async fn from_connection(conn: zbus::Connection) -> Result<Self, WifiBackendError> {
        let owner = identity::owner(&conn, "net.connman.iwd").await?;
        Ok(Self {
            conn,
            owner,
            targets: Mutex::new(Targets::default()),
            cleanup: cleanup::Cleanup::default(),
        })
    }

    async fn verify_owner(&self) -> Result<(), WifiBackendError> {
        verify_control(&self.conn, crate::network::bundle::Kind::Iwd, &self.owner).await
    }

    async fn snapshot(&self) -> Result<WifiSnapshot, WifiBackendError> {
        self.verify_owner().await?;
        let Some(station_path) = self.station_path().await? else {
            self.targets.lock().unwrap().clear();
            return Ok(WifiSnapshot {
                device_id: None,
                networks: Vec::new(),
                scanning: false,
            });
        };
        let station = IwdStationProxy::builder(&self.conn)
            .destination(self.owner.clone())
            .map_err(connection_error)?
            .path(station_path.clone())
            .map_err(scan_error)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .map_err(scan_error)?;
        let ordered = station.get_ordered_networks().await.map_err(scan_error)?;
        let connected = matches!(
            station.state().await.as_deref(),
            Ok("connected" | "roaming")
        );
        let scanning = station.scanning().await.unwrap_or(false);
        let connected_network = station
            .connected_network()
            .await
            .unwrap_or_else(|_| root_object_path());
        let mut networks = Vec::new();
        for (network_path, signal) in ordered {
            let network = IwdNetworkProxy::builder(&self.conn)
                .destination(self.owner.clone())
                .map_err(connection_error)?
                .path(network_path.clone())
                .map_err(scan_error)?
                .cache_properties(zbus::proxy::CacheProperties::No)
                .build()
                .await
                .map_err(scan_error)?;
            let label = network.name().await.unwrap_or_default();
            if label.is_empty() {
                continue;
            }
            let active = connected && network_path == connected_network;
            let known_path = network
                .known_network()
                .await
                .unwrap_or_else(|_| root_object_path());
            let kind = network.network_type().await.map_err(scan_error)?;
            if network.device().await.map_err(scan_error)? != station_path {
                continue;
            }
            networks.push(Observation {
                identity: NetworkIdentity::Iwd {
                    name: label.clone(),
                    kind: kind.clone(),
                },
                label,
                signal_strength: crate::network::iwd::strength(signal),
                security: match kind.as_str() {
                    "open" => WifiSecurity::Open,
                    "psk" => WifiSecurity::Personal,
                    _ => WifiSecurity::Enterprise,
                },
                known: active || known_path.as_str() != "/",
                active,
            });
        }
        self.verify_owner().await?;
        Ok(self.targets.lock().unwrap().publish(
            DeviceIdentity::new(&self.conn, "net.connman.iwd", &self.owner, station_path),
            networks,
            scanning,
        ))
    }

    async fn scan(&self) -> Result<WifiSnapshot, WifiBackendError> {
        let path = self
            .station_path()
            .await?
            .ok_or_else(|| WifiBackendError::new(WifiBackendErrorCategory::NoControl))?;
        let station = IwdStationProxy::builder(&self.conn)
            .destination(self.owner.clone())
            .map_err(connection_error)?
            .path(path)
            .map_err(scan_error)?
            .build()
            .await
            .map_err(scan_error)?;
        let mut changes = station.receive_scanning_changed().await;
        station.scan().await.map_err(scan_error)?;
        tokio::time::timeout(std::time::Duration::from_secs(15), async {
            // Scan schedules work. Wait for its property notification before reading results.
            loop {
                let change = changes
                    .next()
                    .await
                    .ok_or_else(|| WifiBackendError::new(WifiBackendErrorCategory::ScanFailed))?;
                if !change.get().await.map_err(scan_error)? {
                    return Ok::<_, WifiBackendError>(());
                }
            }
        })
        .await
        .map_err(|_| WifiBackendError::new(WifiBackendErrorCategory::ScanFailed))??;
        self.snapshot().await
    }

    async fn connect(&self, request: WifiConnectRequest) -> Result<WifiSnapshot, WifiBackendError> {
        validate_credential(&request)?;
        let (device, network) = self.targets.lock().unwrap().resolve(&request)?;
        self.verify_owner().await?;
        let station_path = device.path;
        let manager = IwdObjectManagerProxy::builder(&self.conn)
            .destination(self.owner.clone())
            .map_err(connection_error)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .map_err(connection_error)?;
        if !manager
            .get_managed_objects()
            .await
            .map_err(connection_error)?
            .get(&station_path)
            .is_some_and(|interfaces| interfaces.contains_key("net.connman.iwd.Station"))
        {
            return Err(identity::changed());
        }
        let station = IwdStationProxy::builder(&self.conn)
            .destination(self.owner.clone())
            .map_err(connection_error)?
            .path(station_path.clone())
            .map_err(connection_error)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .map_err(connection_error)?;
        let (name, kind, path) = if request.hidden {
            (request.label.clone(), "psk".to_owned(), None)
        } else {
            let Some(NetworkIdentity::Iwd { name, kind }) = network else {
                return Err(identity::changed());
            };
            let mut matched = None;
            for (path, _) in station
                .get_ordered_networks()
                .await
                .map_err(connection_error)?
            {
                let proxy = IwdNetworkProxy::builder(&self.conn)
                    .destination(self.owner.clone())
                    .map_err(connection_error)?
                    .path(path.clone())
                    .map_err(connection_error)?
                    .cache_properties(zbus::proxy::CacheProperties::No)
                    .build()
                    .await
                    .map_err(connection_error)?;
                if proxy.name().await.ok().as_ref() == Some(&name)
                    && proxy.network_type().await.ok().as_ref() == Some(&kind)
                    && proxy.device().await.ok().as_ref() == Some(&station_path)
                {
                    matched = Some(path);
                    break;
                }
            }
            (name, kind, Some(matched.ok_or_else(identity::changed)?))
        };
        self.verify_owner().await?;
        let agent = if request.credential.is_some() {
            Some(
                IwdAgentRegistration::register(
                    &self.conn,
                    &self.owner,
                    request.credential.clone(),
                    &station_path,
                    &name,
                    &kind,
                    self.cleanup.clone(),
                )
                .await?,
            )
        } else {
            None
        };
        let result = tokio::time::timeout(std::time::Duration::from_secs(60), async {
            self.verify_owner().await?;
            if let Some(path) = path {
                let proxy = IwdNetworkProxy::builder(&self.conn)
                    .destination(self.owner.clone())
                    .map_err(connection_error)?
                    .path(path)
                    .map_err(connection_error)?
                    .cache_properties(zbus::proxy::CacheProperties::No)
                    .build()
                    .await
                    .map_err(connection_error)?;
                if proxy.name().await.ok().as_ref() != Some(&name)
                    || proxy.network_type().await.ok().as_ref() != Some(&kind)
                    || proxy.device().await.ok().as_ref() != Some(&station_path)
                {
                    return Err(identity::changed());
                }
                proxy.connect().await.map_err(connection_error)?;
            } else {
                station
                    .connect_hidden_network(&name)
                    .await
                    .map_err(connection_error)?;
            }
            Ok::<_, WifiBackendError>(())
        })
        .await
        .map_err(|_| WifiBackendError::new(WifiBackendErrorCategory::ConnectionFailed))?;
        drop(agent);
        result?;
        self.snapshot().await
    }

    async fn station_path(&self) -> Result<Option<OwnedObjectPath>, WifiBackendError> {
        self.verify_owner().await?;
        let manager = IwdObjectManagerProxy::builder(&self.conn)
            .destination(self.owner.clone())
            .map_err(connection_error)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .map_err(scan_error)?;
        let objects = manager.get_managed_objects().await.map_err(scan_error)?;
        let mut stations: Vec<_> = objects
            .into_iter()
            .filter_map(|(path, interfaces)| {
                let properties = interfaces.get("net.connman.iwd.Station")?;
                let active = properties
                    .get("State")
                    .and_then(|v| <&str>::try_from(v).ok())
                    .is_some_and(|state| matches!(state, "connected" | "roaming"));
                Some((!active, path))
            })
            .collect();
        stations.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.as_str().cmp(b.1.as_str())));
        Ok(stations.into_iter().next().map(|(_, path)| path))
    }
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait NmManager {
    fn deactivate_connection(&self, active: &OwnedObjectPath) -> zbus::Result<()>;
    fn activate_connection(
        &self,
        connection: &OwnedObjectPath,
        device: &OwnedObjectPath,
        specific_object: &OwnedObjectPath,
    ) -> zbus::Result<OwnedObjectPath>;
    fn add_and_activate_connection2(
        &self,
        connection: Settings,
        device: &OwnedObjectPath,
        specific_object: &OwnedObjectPath,
        options: HashMap<String, Value<'_>>,
    ) -> zbus::Result<(
        OwnedObjectPath,
        OwnedObjectPath,
        HashMap<String, OwnedValue>,
    )>;
    fn get_devices(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Device",
    default_service = "org.freedesktop.NetworkManager"
)]
trait NmDevice {
    #[zbus(property)]
    fn managed(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn available_connections(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
    #[zbus(property)]
    fn device_type(&self) -> zbus::Result<u32>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wireless",
    default_service = "org.freedesktop.NetworkManager"
)]
trait NmWirelessDevice {
    fn request_scan(&self, options: HashMap<String, Value<'_>>) -> zbus::Result<()>;
    #[zbus(property)]
    fn last_scan(&self) -> zbus::Result<i64>;
    #[zbus(property)]
    fn access_points(&self) -> zbus::Result<Vec<OwnedObjectPath>>;

    #[zbus(property)]
    fn active_access_point(&self) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.AccessPoint",
    default_service = "org.freedesktop.NetworkManager"
)]
trait NmAccessPoint {
    #[zbus(property)]
    fn flags(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn ssid(&self) -> zbus::Result<Vec<u8>>;

    #[zbus(property)]
    fn strength(&self) -> zbus::Result<u8>;

    #[zbus(property)]
    fn wpa_flags(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn rsn_flags(&self) -> zbus::Result<u32>;
}

fn scan_error(error: zbus::Error) -> WifiBackendError {
    let mapped = connection_error(error);
    if mapped.category() == WifiBackendErrorCategory::ConnectionFailed {
        WifiBackendError::new(WifiBackendErrorCategory::ScanFailed)
    } else {
        mapped
    }
}

fn connection_error(error: impl Into<zbus::Error>) -> WifiBackendError {
    let error = error.into();
    let name = match &error {
        zbus::Error::MethodError(name, _, _) => name.as_str(),
        _ => "",
    };
    let category = if name.ends_with("AccessDenied")
        || name.ends_with("PermissionDenied")
        || name.ends_with("NotAuthorized")
    {
        WifiBackendErrorCategory::AuthorizationRequired
    } else if name.ends_with("NoSecrets")
        || name.ends_with("NotConfigured")
        || name.ends_with("NoAgent")
    {
        WifiBackendErrorCategory::CredentialRejected
    } else if name.ends_with("NotSupported") {
        WifiBackendErrorCategory::UnsupportedNetwork
    } else {
        WifiBackendErrorCategory::ConnectionFailed
    };
    WifiBackendError::new(category)
}

fn root_object_path() -> OwnedObjectPath {
    OwnedObjectPath::try_from("/").expect("root object path is valid")
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Settings.Connection",
    default_service = "org.freedesktop.NetworkManager"
)]
trait NmSettingsConnection {
    fn get_settings(&self) -> zbus::Result<Settings>;
    fn save(&self) -> zbus::Result<()>;
    fn delete(&self) -> zbus::Result<()>;
}
#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Connection.Active",
    default_service = "org.freedesktop.NetworkManager"
)]
trait NmActive {
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
    #[zbus(signal, name = "StateChanged")]
    fn activation_changed(&self, state: u32, reason: u32) -> zbus::Result<()>;
}

fn validate_credential(request: &WifiConnectRequest) -> Result<(), WifiBackendError> {
    if let Some(secret) = &request.credential
        && !((8..=63).contains(&secret.len())
            || (secret.len() == 64 && secret.bytes().all(|b| b.is_ascii_hexdigit())))
    {
        return Err(WifiBackendError::new(
            WifiBackendErrorCategory::CredentialRejected,
        ));
    }
    if request.hidden && (request.label.is_empty() || request.label.len() > 32) {
        return Err(WifiBackendError::new(
            WifiBackendErrorCategory::UnsupportedNetwork,
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "backends_tests.rs"]
mod tests;
