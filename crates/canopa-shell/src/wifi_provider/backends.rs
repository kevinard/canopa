#[path = "iwd_agent.rs"]
mod iwd_agent;
use iwd_agent::IwdAgentRegistration;
// D-Bus Wi-Fi adapters. Wire methods follow the NetworkManager and iwd APIs;
// all constructors accept an existing connection internally for isolated tests.
use super::*;
use iced::futures::StreamExt;
use std::collections::HashMap;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

pub async fn choose_backend() -> Option<RuntimeBackend> {
    match std::env::var("CANOPA_WIFI_PROVIDER_BACKEND")
        .or_else(|_| std::env::var("NIRIBAR_WIFI_PROVIDER_BACKEND"))
        .as_deref()
    {
        Ok("none") => return None,
        Ok("networkmanager") => {
            return NetworkManagerBackend::new()
                .await
                .map(RuntimeBackend::NetworkManager);
        }
        Ok("iwd") => return IwdBackend::new().await.map(RuntimeBackend::Iwd),
        _ => {}
    }

    let availability = detect_backend_availability().await?;
    match select_backend(availability) {
        Some(BackendName::NetworkManager) => NetworkManagerBackend::new()
            .await
            .map(RuntimeBackend::NetworkManager),
        Some(BackendName::Iwd) => IwdBackend::new().await.map(RuntimeBackend::Iwd),
        None => None,
    }
}

async fn detect_backend_availability() -> Option<WifiBackendAvailability> {
    let conn = zbus::Connection::system().await.ok()?;
    let dbus = zbus::fdo::DBusProxy::new(&conn).await.ok()?;
    let networkmanager = dbus
        .name_has_owner("org.freedesktop.NetworkManager".try_into().ok()?)
        .await
        .unwrap_or(false);
    let iwd = dbus
        .name_has_owner("net.connman.iwd".try_into().ok()?)
        .await
        .unwrap_or(false);
    Some(WifiBackendAvailability {
        networkmanager,
        iwd,
    })
}

pub enum RuntimeBackend {
    NetworkManager(NetworkManagerBackend),
    Iwd(IwdBackend),
}

impl RuntimeBackend {
    pub fn cleanup(&self) -> cleanup::Cleanup {
        match self {
            Self::NetworkManager(b) => b.cleanup.clone(),
            Self::Iwd(b) => b.cleanup.clone(),
        }
    }
    pub async fn changes(&self) -> zbus::Result<tokio::sync::mpsc::Receiver<()>> {
        let (conn, service) = match self {
            Self::NetworkManager(backend) => (&backend.conn, "org.freedesktop.NetworkManager"),
            Self::Iwd(backend) => (&backend.conn, "net.connman.iwd"),
        };
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .sender(service)?
            .build();
        let mut signals = zbus::MessageStream::for_match_rule(rule, conn, Some(64)).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tx.closed() => break,
                    signal = signals.next() => match signal {
                        Some(Ok(_)) => { let _ = tx.try_send(()); }
                        _ => break,
                    }
                }
            }
        });
        Ok(rx)
    }
}

impl WifiControlBackend for RuntimeBackend {
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
}

pub struct NetworkManagerBackend {
    cleanup: cleanup::Cleanup,
    conn: zbus::Connection,
}

impl NetworkManagerBackend {
    async fn new() -> Option<Self> {
        Some(Self {
            cleanup: cleanup::Cleanup::default(),
            conn: zbus::Connection::system().await.ok()?,
        })
    }

    async fn device(&self) -> Result<OwnedObjectPath, WifiBackendError> {
        let manager = NmManagerProxy::new(&self.conn).await.map_err(scan_error)?;
        let mut candidates = Vec::new();
        for path in manager.get_devices().await.map_err(scan_error)? {
            let device = NmDeviceProxy::builder(&self.conn)
                .path(path.clone())
                .map_err(scan_error)?
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
            .path(device.clone())
            .map_err(scan_error)?
            .build()
            .await
            .map_err(scan_error)?;
        let mut profiles = Vec::new();
        for path in device.available_connections().await.map_err(scan_error)? {
            let proxy = NmSettingsConnectionProxy::builder(&self.conn)
                .path(path.clone())
                .map_err(scan_error)?
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
            .path(device_path.clone())
            .map_err(scan_error)?
            .build()
            .await
            .map_err(scan_error)?;
        let connected = device.state().await.unwrap_or(0) == 100;
        let wireless = NmWirelessDeviceProxy::builder(&self.conn)
            .path(device_path)
            .map_err(scan_error)?
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
                .path(path.clone())
                .map_err(scan_error)?
                .build()
                .await
                .map_err(scan_error)?;
            let Ok(ssid) = proxy.ssid().await else {
                continue;
            };
            if ssid.is_empty() {
                continue;
            }
            let flags = proxy.wpa_flags().await.unwrap_or(0) | proxy.rsn_flags().await.unwrap_or(0);
            let security = nm_security(flags, proxy.flags().await.unwrap_or(0));
            let active = connected && path == active_access_point;
            networks.push(WifiNetwork {
                id: path.to_string(),
                label: String::from_utf8_lossy(&ssid).into_owned(),
                signal_strength: proxy.strength().await.unwrap_or(0),
                security,
                known: profiles
                    .iter()
                    .any(|(_, saved, key)| *saved == ssid && profile_security(key) == security),
                active,
            });
        }
        Ok(WifiSnapshot {
            networks,
            scanning: false,
        })
    }

    async fn scan(&self) -> Result<WifiSnapshot, WifiBackendError> {
        let path = self.device().await?;
        let wireless = NmWirelessDeviceProxy::builder(&self.conn)
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
        let device = self.device().await?;
        let manager = NmManagerProxy::new(&self.conn)
            .await
            .map_err(connection_error)?;
        let ap = request
            .network_id
            .as_deref()
            .map(OwnedObjectPath::try_from)
            .transpose()
            .map_err(connection_error)?
            .unwrap_or_else(root_object_path);
        let (ssid, security, key_mgmt) = if request.hidden {
            (
                request.label.as_bytes().to_vec(),
                WifiSecurity::Personal,
                "wpa-psk",
            )
        } else {
            let proxy = NmAccessPointProxy::builder(&self.conn)
                .path(ap.clone())
                .map_err(connection_error)?
                .build()
                .await
                .map_err(connection_error)?;
            let flags = proxy.wpa_flags().await.unwrap_or(0) | proxy.rsn_flags().await.unwrap_or(0);
            (
                proxy.ssid().await.map_err(connection_error)?,
                nm_security(flags, proxy.flags().await.unwrap_or(0)),
                if flags & 0x100 == 0 && flags & 0x400 != 0 {
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
            let cleanup = self.cleanup.clone();
            let (tx, rx) = tokio::sync::oneshot::channel();
            // Let the request finish even if its UI future is canceled: the
            // resulting guard must own the daemon-side resource before cleanup.
            self.cleanup.spawn(async move {
                let result = tokio::time::timeout(std::time::Duration::from_secs(30), async {
                    let manager = NmManagerProxy::new(&conn).await.map_err(connection_error)?;
                    let options = HashMap::from([("persist".to_string(), Value::from("memory"))]);
                    let (profile, active, _) = manager
                        .add_and_activate_connection2(settings, &device, &ap, options)
                        .await
                        .map_err(connection_error)?;
                    Ok::<_, WifiBackendError>(NmPendingProfile {
                        conn: conn.clone(),
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
                .path(pending.resource.as_ref().unwrap().0.clone())
                .map_err(connection_error)?
                .build()
                .await
                .map_err(connection_error)?;
            proxy.save().await.map_err(connection_error)?;
            pending.resource = None;
        }
        result?;
        self.snapshot().await
    }

    async fn wait_for_activation(&self, path: OwnedObjectPath) -> Result<(), WifiBackendError> {
        let proxy = NmActiveProxy::builder(&self.conn)
            .path(path)
            .map_err(connection_error)?
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
        self.cleanup.spawn(async move {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                if let Ok(manager) = NmManagerProxy::new(&conn).await {
                    let _ = manager.deactivate_connection(&active).await;
                }
                if let Ok(proxy) = NmSettingsConnectionProxy::builder(&conn)
                    .path(profile)
                    .unwrap()
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

type Settings = HashMap<String, HashMap<String, OwnedValue>>;
fn owned(value: impl Into<Value<'static>>) -> OwnedValue {
    value
        .into()
        .try_into()
        .expect("owned scalar/array has no file descriptors")
}
fn nm_settings(
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
    cleanup: cleanup::Cleanup,
    conn: zbus::Connection,
}

impl IwdBackend {
    async fn new() -> Option<Self> {
        Some(Self {
            cleanup: cleanup::Cleanup::default(),
            conn: zbus::Connection::system().await.ok()?,
        })
    }

    async fn snapshot(&self) -> Result<WifiSnapshot, WifiBackendError> {
        let Some(station_path) = self.station_path().await? else {
            return Ok(WifiSnapshot {
                networks: Vec::new(),
                scanning: false,
            });
        };
        let station = IwdStationProxy::builder(&self.conn)
            .path(station_path)
            .map_err(scan_error)?
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
                .path(network_path.clone())
                .map_err(scan_error)?
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
            networks.push(WifiNetwork {
                id: network_path.to_string(),
                label,
                signal_strength: normalize_iwd_signal(signal),
                security: match network.network_type().await.map_err(scan_error)?.as_str() {
                    "open" => WifiSecurity::Open,
                    "psk" => WifiSecurity::Personal,
                    _ => WifiSecurity::Enterprise,
                },
                known: active || known_path.as_str() != "/",
                active,
            });
        }
        Ok(WifiSnapshot { networks, scanning })
    }

    async fn scan(&self) -> Result<WifiSnapshot, WifiBackendError> {
        let path = self
            .station_path()
            .await?
            .ok_or_else(|| WifiBackendError::new(WifiBackendErrorCategory::NoControl))?;
        let station = IwdStationProxy::builder(&self.conn)
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
        let station_path = self
            .station_path()
            .await?
            .ok_or_else(|| WifiBackendError::new(WifiBackendErrorCategory::NoControl))?;
        let agent = if request.credential.is_some() {
            Some(
                IwdAgentRegistration::register(
                    &self.conn,
                    &request,
                    &station_path,
                    self.cleanup.clone(),
                )
                .await?,
            )
        } else {
            None
        };
        let result = tokio::time::timeout(std::time::Duration::from_secs(60), async {
            if request.hidden {
                let station = IwdStationProxy::builder(&self.conn)
                    .path(station_path)
                    .map_err(connection_error)?
                    .build()
                    .await
                    .map_err(connection_error)?;
                station
                    .connect_hidden_network(&request.label)
                    .await
                    .map_err(connection_error)?;
            } else {
                let path = request.network_id.as_deref().ok_or_else(|| {
                    WifiBackendError::new(WifiBackendErrorCategory::UnsupportedNetwork)
                })?;
                let network = IwdNetworkProxy::builder(&self.conn)
                    .path(path)
                    .map_err(connection_error)?
                    .build()
                    .await
                    .map_err(connection_error)?;
                network.connect().await.map_err(connection_error)?;
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
        let manager = IwdObjectManagerProxy::new(&self.conn)
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

type ManagedObjects =
    HashMap<OwnedObjectPath, HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>>>;

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

#[zbus::proxy(
    interface = "org.freedesktop.DBus.ObjectManager",
    default_service = "net.connman.iwd",
    default_path = "/"
)]
trait IwdObjectManager {
    fn get_managed_objects(&self) -> zbus::Result<ManagedObjects>;
}

#[zbus::proxy(
    interface = "net.connman.iwd.Station",
    default_service = "net.connman.iwd"
)]
trait IwdStation {
    fn connect_hidden_network(&self, ssid: &str) -> zbus::Result<()>;
    #[zbus(property)]
    fn state(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn scanning(&self) -> zbus::Result<bool>;
    fn scan(&self) -> zbus::Result<()>;
    fn get_ordered_networks(&self) -> zbus::Result<Vec<(OwnedObjectPath, i16)>>;

    #[zbus(property)]
    fn connected_network(&self) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "net.connman.iwd.Network",
    default_service = "net.connman.iwd"
)]
trait IwdNetwork {
    #[zbus(property, name = "Type")]
    fn network_type(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn device(&self) -> zbus::Result<OwnedObjectPath>;
    fn connect(&self) -> zbus::Result<()>;

    #[zbus(property)]
    fn name(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn connected(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn known_network(&self) -> zbus::Result<OwnedObjectPath>;
}

fn normalize_iwd_signal(signal: i16) -> u8 {
    ((i32::from(signal) + 10000).clamp(0, 10000) / 100) as u8
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
