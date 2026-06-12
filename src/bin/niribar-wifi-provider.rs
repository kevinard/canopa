use std::collections::HashMap;

use niribar::external_panel::protocol::{
    ExternalPanelDocument, ExternalPanelElement, ExternalPanelSeverity, HostMessage,
    PROTOCOL_VERSION, ProviderEvent, ProviderMessage, read_frame, write_frame,
};
use niribar::wifi_provider::{
    BackendCapabilities, BackendName, WifiBackendAvailability, WifiBackendError,
    WifiBackendErrorCategory, WifiBackendFuture, WifiConnectRequest, WifiControlBackend,
    WifiNetwork, WifiPanelProvider, WifiSecurity, WifiSnapshot, select_backend,
};
use zbus::zvariant::OwnedObjectPath;

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("niribar-wifi-provider: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let HostMessage::Handshake(handshake) = read_frame::<_, HostMessage>(&mut stdin).await? else {
        return Err("expected host handshake".into());
    };
    if handshake.protocol_version != PROTOCOL_VERSION {
        return Err(format!(
            "unsupported external panel protocol version {}",
            handshake.protocol_version
        )
        .into());
    }

    let Some(backend) = choose_backend().await else {
        write_provider_message(
            &mut stdout,
            ProviderMessage::Handshake {
                protocol_version: PROTOCOL_VERSION,
                content: niribar::wifi_provider::no_control_document(),
            },
        )
        .await?;
        return drain_until_close(stdin).await;
    };

    let mut provider = WifiPanelProvider::new(backend);
    let initial = match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        provider.initial_content(),
    )
    .await
    {
        Ok(Ok(content)) => content,
        Ok(Err(err)) => backend_error_document("Wi-Fi unavailable", err.category()),
        Err(_) => backend_error_document("Wi-Fi unavailable", WifiBackendErrorCategory::NoControl),
    };
    write_provider_message(
        &mut stdout,
        ProviderMessage::Handshake {
            protocol_version: PROTOCOL_VERSION,
            content: initial,
        },
    )
    .await?;

    if let Ok(Ok(content)) =
        tokio::time::timeout(std::time::Duration::from_secs(10), provider.refresh()).await
    {
        write_provider_message(&mut stdout, ProviderMessage::Content { content }).await?;
    }

    loop {
        match read_frame::<_, HostMessage>(&mut stdin).await {
            Ok(HostMessage::Event(ProviderEvent::Close)) => return Ok(()),
            Ok(HostMessage::Event(event)) => match provider.handle_event(event).await {
                Ok(documents) => {
                    for content in documents {
                        write_provider_message(&mut stdout, ProviderMessage::Content { content })
                            .await?;
                    }
                }
                Err(err) => {
                    write_provider_message(
                        &mut stdout,
                        ProviderMessage::Content {
                            content: backend_error_document("Wi-Fi problem", err.category()),
                        },
                    )
                    .await?;
                }
            },
            Ok(HostMessage::Handshake(_)) => return Err("unexpected repeated handshake".into()),
            Err(_) => return Ok(()),
        }
    }
}

async fn write_provider_message<W>(
    writer: &mut W,
    message: ProviderMessage,
) -> Result<(), niribar::external_panel::protocol::ProtocolError>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    write_frame(writer, &message).await
}

async fn drain_until_close(mut stdin: tokio::io::Stdin) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        match read_frame::<_, HostMessage>(&mut stdin).await {
            Ok(HostMessage::Event(ProviderEvent::Close)) => return Ok(()),
            Ok(HostMessage::Event(_)) => {}
            Ok(HostMessage::Handshake(_)) => return Err("unexpected repeated handshake".into()),
            Err(_) => return Ok(()),
        }
    }
}

async fn choose_backend() -> Option<RuntimeBackend> {
    match std::env::var("NIRIBAR_WIFI_PROVIDER_BACKEND").as_deref() {
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

fn backend_error_document(
    title: &str,
    category: WifiBackendErrorCategory,
) -> ExternalPanelDocument {
    ExternalPanelDocument {
        elements: vec![ExternalPanelElement::Status {
            title: title.into(),
            message: category_message(category).into(),
            severity: ExternalPanelSeverity::Error,
        }],
    }
}

fn category_message(category: WifiBackendErrorCategory) -> &'static str {
    match category {
        WifiBackendErrorCategory::NoControl => "No supported Wi-Fi control backend is available.",
        WifiBackendErrorCategory::ScanFailed => "Wi-Fi scan failed.",
        WifiBackendErrorCategory::ConnectionFailed => "Connection failed.",
        WifiBackendErrorCategory::CredentialRejected => "Network credential was rejected.",
        WifiBackendErrorCategory::UnsupportedNetwork => "This Wi-Fi network is not supported.",
        WifiBackendErrorCategory::AuthorizationRequired => "Wi-Fi authorization is required.",
    }
}

enum RuntimeBackend {
    NetworkManager(NetworkManagerBackend),
    Iwd(IwdBackend),
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

struct NetworkManagerBackend {
    conn: zbus::Connection,
}

impl NetworkManagerBackend {
    async fn new() -> Option<Self> {
        Some(Self {
            conn: zbus::Connection::system().await.ok()?,
        })
    }

    async fn snapshot(&self) -> Result<WifiSnapshot, WifiBackendError> {
        let manager = NmManagerProxy::new(&self.conn).await.map_err(scan_error)?;
        let devices = manager.get_devices().await.map_err(scan_error)?;
        let mut networks = Vec::new();

        for device_path in devices {
            let device = NmDeviceProxy::builder(&self.conn)
                .path(device_path.clone())
                .map_err(scan_error)?
                .build()
                .await
                .map_err(scan_error)?;
            if device.device_type().await.unwrap_or(0) != 2 {
                continue;
            }
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
            for access_point in wireless.access_points().await.unwrap_or_default() {
                let proxy = NmAccessPointProxy::builder(&self.conn)
                    .path(access_point.clone())
                    .map_err(scan_error)?
                    .build()
                    .await
                    .map_err(scan_error)?;
                let ssid = String::from_utf8_lossy(&proxy.ssid().await.unwrap_or_default())
                    .trim_matches(char::from(0))
                    .to_string();
                if ssid.is_empty() {
                    continue;
                }
                let active = access_point == active_access_point;
                let secured = proxy.wpa_flags().await.unwrap_or(0) != 0
                    || proxy.rsn_flags().await.unwrap_or(0) != 0;
                networks.push(WifiNetwork {
                    id: access_point.to_string(),
                    label: ssid,
                    signal_strength: proxy.strength().await.unwrap_or(0),
                    security: if secured {
                        WifiSecurity::Personal
                    } else {
                        WifiSecurity::Open
                    },
                    known: active,
                    active,
                });
            }
        }

        Ok(WifiSnapshot {
            networks,
            scanning: false,
        })
    }

    async fn scan(&self) -> Result<WifiSnapshot, WifiBackendError> {
        self.snapshot().await
    }

    async fn connect(
        &self,
        _request: WifiConnectRequest,
    ) -> Result<WifiSnapshot, WifiBackendError> {
        Err(WifiBackendError::new(
            WifiBackendErrorCategory::AuthorizationRequired,
        ))
    }
}

struct IwdBackend {
    conn: zbus::Connection,
}

impl IwdBackend {
    async fn new() -> Option<Self> {
        Some(Self {
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
        let ordered = station.get_ordered_networks().await.unwrap_or_default();
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
            let active =
                network_path == connected_network || network.connected().await.unwrap_or(false);
            let known_path = network
                .known_network()
                .await
                .unwrap_or_else(|_| root_object_path());
            networks.push(WifiNetwork {
                id: network_path.to_string(),
                label,
                signal_strength: normalize_iwd_signal(signal),
                security: WifiSecurity::Personal,
                known: active || known_path.as_str() != "/",
                active,
            });
        }
        Ok(WifiSnapshot {
            networks,
            scanning: false,
        })
    }

    async fn scan(&self) -> Result<WifiSnapshot, WifiBackendError> {
        if let Some(station_path) = self.station_path().await? {
            let station = IwdStationProxy::builder(&self.conn)
                .path(station_path)
                .map_err(scan_error)?
                .build()
                .await
                .map_err(scan_error)?;
            let _ = station.scan().await;
        }
        self.snapshot().await
    }

    async fn connect(&self, request: WifiConnectRequest) -> Result<WifiSnapshot, WifiBackendError> {
        if request.hidden || request.credential.is_some() {
            return Err(WifiBackendError::new(
                WifiBackendErrorCategory::UnsupportedNetwork,
            ));
        }
        let Some(path) = request.network_id else {
            return Err(WifiBackendError::new(
                WifiBackendErrorCategory::UnsupportedNetwork,
            ));
        };
        let object_path = OwnedObjectPath::try_from(path)
            .map_err(|_| WifiBackendError::new(WifiBackendErrorCategory::ConnectionFailed))?;
        let network = IwdNetworkProxy::builder(&self.conn)
            .path(object_path)
            .map_err(connection_error)?
            .build()
            .await
            .map_err(connection_error)?;
        network.connect().await.map_err(connection_error)?;
        self.snapshot().await
    }

    async fn station_path(&self) -> Result<Option<OwnedObjectPath>, WifiBackendError> {
        let manager = IwdObjectManagerProxy::new(&self.conn)
            .await
            .map_err(scan_error)?;
        let objects = manager.get_managed_objects().await.map_err(scan_error)?;
        for (path, interfaces) in objects {
            if interfaces.contains_key("net.connman.iwd.Station") {
                return Ok(Some(path));
            }
        }
        Ok(None)
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
    fn get_devices(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
}

#[zbus::proxy(interface = "org.freedesktop.NetworkManager.Device")]
trait NmDevice {
    #[zbus(property)]
    fn device_type(&self) -> zbus::Result<u32>;
}

#[zbus::proxy(interface = "org.freedesktop.NetworkManager.Device.Wireless")]
trait NmWirelessDevice {
    #[zbus(property)]
    fn access_points(&self) -> zbus::Result<Vec<OwnedObjectPath>>;

    #[zbus(property)]
    fn active_access_point(&self) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(interface = "org.freedesktop.NetworkManager.AccessPoint")]
trait NmAccessPoint {
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
    fn connect(&self) -> zbus::Result<()>;

    #[zbus(property)]
    fn name(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn connected(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn known_network(&self) -> zbus::Result<OwnedObjectPath>;
}

fn normalize_iwd_signal(signal: i16) -> u8 {
    if signal < 0 {
        (signal + 100).clamp(0, 100) as u8
    } else {
        signal.clamp(0, 100) as u8
    }
}

fn scan_error(error: zbus::Error) -> WifiBackendError {
    let _ = error;
    WifiBackendError::new(WifiBackendErrorCategory::ScanFailed)
}

fn connection_error(error: zbus::Error) -> WifiBackendError {
    let _ = error;
    WifiBackendError::new(WifiBackendErrorCategory::ConnectionFailed)
}

fn root_object_path() -> OwnedObjectPath {
    OwnedObjectPath::try_from("/").expect("root object path is valid")
}
