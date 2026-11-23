//! The Network page's service: NetworkManager's devices, nearby networks and
//! saved connections, the changes the page asks for, and the secret agent
//! that relays NetworkManager's questions to the page (ADR 0139).
use crate::live::{LiveAction, LiveService, ServiceError};
use canopa_services::network::{
    manager::{self, AccessPoint, Change, Overview, Security, Target},
    profiles::{self, WifiSecurity, WireGuard},
    secret_agent::{self, SecretField, SecretRequest},
};
use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

/// A question NetworkManager asked through the agent, as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Question {
    pub connection: String,
    pub setting: String,
    pub fields: Vec<SecretField>,
    pub retry: bool,
}

/// What the page shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkState {
    pub overview: Overview,
    pub question: Option<Question>,
    /// The secret agent could not register; questions cannot be answered.
    pub agent: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub enum NetworkAction {
    WifiEnabled(bool),
    /// Connect a nearby network that is saved or open.
    Connect(AccessPoint),
    /// Join a network with new settings; `device` is the Wi-Fi device.
    Join {
        ssid: Vec<u8>,
        security: WifiSecurity,
        hidden: bool,
        device: String,
    },
    Activate(Target),
    /// Disconnect an active connection.
    Deactivate(String),
    Forget(Target),
    Change(Target, Change),
    AddWireGuard(WireGuard),
    /// A VPN file and, for plugin VPNs, the plugin's short name.
    Import(PathBuf, Option<String>),
    Hotspot {
        ssid: String,
        password: String,
        device: String,
    },
    Answer(HashMap<String, String>),
    Cancel,
}

impl std::fmt::Debug for NetworkAction {
    // Secrets the person typed never reach logs.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.control())
    }
}

impl LiveAction for NetworkAction {
    fn control(&self) -> String {
        match self {
            Self::WifiEnabled(_) => "wifi-enabled".into(),
            Self::Connect(ap) => format!("wifi-{}", ap.name()),
            Self::Join { .. } => "wifi-join".into(),
            Self::Activate(target) | Self::Forget(target) => format!("connection-{}", target.uuid),
            Self::Deactivate(active) => format!("active-{active}"),
            Self::Change(target, change) => format!(
                "change-{}-{}",
                target.uuid,
                match change {
                    Change::Autoconnect(_) => "autoconnect",
                    Change::Metered(_) => "metered",
                    Change::Ip("ipv4", _) => "ipv4",
                    Change::Ip(..) => "ipv6",
                    Change::Proxy(_) => "proxy",
                }
            ),
            Self::AddWireGuard(_) | Self::Import(..) => "vpn-add".into(),
            Self::Hotspot { .. } => "hotspot".into(),
            Self::Answer(_) | Self::Cancel => "secret".into(),
        }
    }
}

fn no_service() -> String {
    fl!("settings-network-not-running")
}

struct Agent {
    _registration: secret_agent::Registration,
}

/// NetworkManager for the page. The secret agent registers on first use and
/// stays registered while the window runs.
#[derive(Clone)]
pub struct NetworkService {
    system: Arc<tokio::sync::OnceCell<zbus::Connection>>,
    agent: Arc<tokio::sync::Mutex<Option<Agent>>>,
    pending: Arc<Mutex<Option<SecretRequest>>>,
    questions: watch::Sender<Option<Question>>,
    plugins: PathBuf,
    nmcli: PathBuf,
}

impl Default for NetworkService {
    fn default() -> Self {
        Self {
            system: Arc::default(),
            agent: Arc::default(),
            pending: Arc::default(),
            questions: watch::channel(None).0,
            plugins: manager::VPN_PLUGINS.into(),
            nmcli: "nmcli".into(),
        }
    }
}

fn failure(error: canopa_services::Error) -> ServiceError {
    use canopa_services::Error;
    match error {
        Error::Unavailable => ServiceError::Unavailable(no_service()),
        Error::NotAuthorized => ServiceError::Failed(fl!("settings-error-not-authorized")),
        Error::Changed => ServiceError::Failed(fl!("settings-network-error-changed")),
        Error::TimedOut => ServiceError::Failed(fl!("settings-network-error-timed-out")),
        Error::Invalid(message) => ServiceError::Failed(crate::service_text::invalid(message)),
        Error::Refused(message) => ServiceError::Failed(message),
    }
}

impl NetworkService {
    /// A service on an existing connection, with VPN plugins under `plugins`
    /// and `nmcli` at `nmcli` (tests).
    pub fn with(system: zbus::Connection, plugins: PathBuf, nmcli: PathBuf) -> Self {
        Self {
            system: Arc::new(tokio::sync::OnceCell::new_with(Some(system))),
            plugins,
            nmcli,
            ..Default::default()
        }
    }

    async fn connection(&self) -> Result<zbus::Connection, ServiceError> {
        self.system
            .get_or_try_init(canopa_services::bus::system)
            .await
            .cloned()
            .map_err(|_| ServiceError::Unavailable(fl!("settings-error-bus")))
    }

    /// Register the agent once, and relay its questions to the page.
    async fn agent(&self, system: &zbus::Connection) -> bool {
        let mut agent = self.agent.lock().await;
        if agent.is_some() {
            return true;
        }
        let Ok((registration, mut requests)) =
            secret_agent::register(system, "org.canopa.Settings").await
        else {
            return false;
        };
        let pending = self.pending.clone();
        let questions = self.questions.clone();
        tokio::spawn(async move {
            while let Some(request) = requests.recv().await {
                questions.send_replace(Some(Question {
                    connection: request.connection.clone(),
                    setting: request.setting.clone(),
                    fields: request.fields.clone(),
                    retry: request.retry,
                }));
                let previous = pending
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .replace(request);
                if let Some(previous) = previous {
                    previous.cancel();
                }
            }
        });
        *agent = Some(Agent {
            _registration: registration,
        });
        true
    }

    async fn read(&self) -> Result<NetworkState, ServiceError> {
        let system = self.connection().await?;
        let overview = manager::read_with_plugins(&system, &self.plugins)
            .await
            .map_err(failure)?;
        let agent = self.agent(&system).await;
        Ok(NetworkState {
            overview,
            question: self.questions.borrow().clone(),
            agent,
        })
    }

    fn take_question(&self) -> Option<SecretRequest> {
        self.questions.send_replace(None);
        self.pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
    }

    async fn import(&self, path: &Path, kind: Option<&str>) -> Result<(), ServiceError> {
        let text = std::fs::read(path)
            .ok()
            .filter(|bytes| bytes.len() <= 256 * 1024)
            .and_then(|bytes| String::from_utf8(bytes).ok());
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("VPN");
        if let Some(text) = &text
            && (kind == Some("wireguard")
                || (kind.is_none() && text.to_ascii_lowercase().contains("[interface]")))
        {
            let tunnel = profiles::parse_wireguard(text, name).map_err(failure)?;
            let settings = profiles::wireguard(&tunnel).map_err(failure)?;
            manager::add(&self.connection().await?, settings)
                .await
                .map_err(failure)?;
            return Ok(());
        }
        let kind =
            kind.ok_or_else(|| ServiceError::Failed(fl!("settings-network-import-choose-kind")))?;
        if !kind
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(ServiceError::Failed(fl!(
                "settings-network-import-unknown-kind"
            )));
        }
        // The D-Bus API cannot import; nmcli uses the installed plugin to read the file.
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            tokio::process::Command::new(&self.nmcli)
                .args(["connection", "import", "type", kind, "file"])
                .arg(path)
                .stdin(std::process::Stdio::null())
                .kill_on_drop(true)
                .output(),
        )
        .await
        .map_err(|_| ServiceError::Failed(fl!("settings-network-import-timed-out")))?
        .map_err(|_| ServiceError::Failed(fl!("settings-network-import-needs-nmcli")))?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr);
            let message = message
                .trim()
                .lines()
                .last()
                .unwrap_or("")
                .chars()
                .take(200);
            let message: String = message.collect();
            return Err(ServiceError::Failed(fl!(
                "settings-network-import-failed",
                error = message
            )));
        }
        Ok(())
    }

    async fn run(&self, action: NetworkAction) -> Result<(), ServiceError> {
        if let NetworkAction::Answer(values) = action {
            return match self.take_question() {
                Some(request) => {
                    request.answer(values);
                    Ok(())
                }
                None => Err(ServiceError::Failed(fl!("settings-network-question-gone"))),
            };
        }
        if matches!(action, NetworkAction::Cancel) {
            if let Some(request) = self.take_question() {
                request.cancel();
            }
            return Ok(());
        }
        let system = self.connection().await?;
        let result = match action {
            NetworkAction::WifiEnabled(enabled) => {
                manager::set_wireless_enabled(&system, enabled).await
            }
            NetworkAction::Connect(ap) => {
                let overview = manager::read(&system).await.map_err(failure)?;
                if let Some(saved) = overview.saved_for(&ap) {
                    manager::activate(&system, &Target::from(saved), &ap.device, &ap.path)
                        .await
                        .map(drop)
                } else if ap.security == Security::Open {
                    let settings =
                        profiles::wifi(&ap.ssid, &WifiSecurity::Open, false).map_err(failure)?;
                    manager::add_and_activate(&system, settings, &ap.device, &ap.path)
                        .await
                        .map(drop)
                } else {
                    return Err(ServiceError::Failed(fl!("settings-network-needs-password")));
                }
            }
            NetworkAction::Join {
                ssid,
                security,
                hidden,
                device,
            } => {
                let settings = profiles::wifi(&ssid, &security, hidden).map_err(failure)?;
                manager::add_and_activate(&system, settings, &device, "/")
                    .await
                    .map(drop)
            }
            NetworkAction::Activate(target) => manager::activate(&system, &target, "/", "/")
                .await
                .map(drop),
            NetworkAction::Deactivate(active) => manager::deactivate(&system, &active).await,
            NetworkAction::Forget(target) => manager::delete(&system, &target).await,
            NetworkAction::Change(target, change) => {
                manager::update(&system, &target, &change).await
            }
            NetworkAction::AddWireGuard(tunnel) => {
                let settings = profiles::wireguard(&tunnel).map_err(failure)?;
                manager::add(&system, settings).await.map(drop)
            }
            NetworkAction::Import(path, kind) => return self.import(&path, kind.as_deref()).await,
            NetworkAction::Hotspot {
                ssid,
                password,
                device,
            } => {
                let settings = profiles::hotspot(&ssid, &password).map_err(failure)?;
                manager::add_and_activate(&system, settings, &device, "/")
                    .await
                    .map(drop)
            }
            NetworkAction::Answer(_) | NetworkAction::Cancel => unreachable!("handled above"),
        };
        result.map_err(failure)
    }
}

enum Part {
    Overview(canopa_services::ServiceState<Overview>),
    Question(Option<Question>),
}

impl LiveService for NetworkService {
    type State = NetworkState;
    type Action = NetworkAction;

    fn name(&self) -> String {
        "NetworkManager".into()
    }

    fn snapshot(&self) -> BoxFuture<'static, Result<NetworkState, ServiceError>> {
        let service = self.clone();
        async move { service.read().await }.boxed()
    }

    fn updates(&self) -> BoxStream<'static, Result<NetworkState, ServiceError>> {
        let service = self.clone();
        futures_util::stream::once(async move {
            let Ok(system) = service.connection().await else {
                return futures_util::stream::iter([Err(ServiceError::Unavailable(fl!(
                    "settings-error-bus"
                )))])
                .boxed();
            };
            let mut questions = service.questions.subscribe();
            questions.mark_unchanged();
            let questions = futures_util::stream::unfold(questions, |mut questions| async move {
                questions.changed().await.ok()?;
                let question = questions.borrow_and_update().clone();
                Some((Part::Question(question), questions))
            });
            let overviews = manager::watch(system).map(Part::Overview);
            let plugins = service.plugins.clone();
            futures_util::stream::select(overviews.boxed(), questions.boxed())
                .scan(
                    (None::<Overview>, service.questions.borrow().clone()),
                    move |(overview, question), part| {
                        let next = match part {
                            Part::Overview(canopa_services::ServiceState::Available(mut value)) => {
                                value.vpn_plugins = manager::vpn_plugins(&plugins);
                                *overview = Some(value);
                                None
                            }
                            Part::Overview(canopa_services::ServiceState::Unavailable) => {
                                Some(Err(ServiceError::Unavailable(no_service())))
                            }
                            Part::Question(value) => {
                                *question = value;
                                None
                            }
                        };
                        let item = next.or_else(|| {
                            overview.clone().map(|overview| {
                                Ok(NetworkState {
                                    overview,
                                    question: question.clone(),
                                    agent: true,
                                })
                            })
                        });
                        std::future::ready(Some(item))
                    },
                )
                .filter_map(std::future::ready)
                .take_while(|item| std::future::ready(item.is_ok()))
                .chain(futures_util::stream::once(async {
                    Err(ServiceError::Unavailable(no_service()))
                }))
                .boxed()
        })
        .flatten()
        .boxed()
    }

    fn apply(
        &self,
        action: NetworkAction,
    ) -> BoxFuture<'static, Result<Option<NetworkState>, ServiceError>> {
        let service = self.clone();
        async move {
            service.run(action).await?;
            Ok(Some(service.read().await?))
        }
        .boxed()
    }
}

/// Ask the desktop portal for a file. `None` when the person cancels.
pub async fn choose_file(title: &str) -> anyhow::Result<Option<PathBuf>> {
    open_file(title, None).await
}

/// Ask the desktop portal for a JPEG or PNG picture.
pub async fn choose_picture(title: &str, filter: &str) -> anyhow::Result<Option<PathBuf>> {
    open_file(title, Some(filter)).await
}

async fn open_file(title: &str, pictures: Option<&str>) -> anyhow::Result<Option<PathBuf>> {
    use zbus::zvariant::{OwnedValue, Value};
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let bus = zbus::Connection::session().await?;
    let token = format!(
        "canopa_settings_{}",
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let sender = bus
        .unique_name()
        .map(|name| name.trim_start_matches(':').replace('.', "_"))
        .unwrap_or_default();
    let request = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    // Subscribe before asking, so the answer cannot arrive first.
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.portal.Request")?
        .member("Response")?
        .path(request.as_str())?
        .build();
    let mut responses = zbus::MessageStream::for_match_rule(rule, &bus, Some(4)).await?;
    let chooser = zbus::Proxy::new(
        &bus,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.FileChooser",
    )
    .await?;
    let mut options = HashMap::from([
        ("handle_token", Value::from(token.as_str())),
        ("modal", Value::from(true)),
    ]);
    if let Some(name) = pictures {
        // One filter of MIME types (kind 1), the only ones a wallpaper reads.
        let filter = (
            name.to_owned(),
            vec![
                (1_u32, "image/jpeg".to_owned()),
                (1, "image/png".to_owned()),
            ],
        );
        options.insert("filters", Value::new(vec![filter.clone()]));
        options.insert("current_filter", Value::new(filter));
    }
    let _: zbus::zvariant::OwnedObjectPath =
        chooser.call("OpenFile", &("", title, options)).await?;
    let Some(message) = responses.next().await.transpose()? else {
        return Ok(None);
    };
    let (code, results): (u32, HashMap<String, OwnedValue>) = message.body().deserialize()?;
    if code != 0 {
        return Ok(None);
    }
    let uri = results
        .get("uris")
        .and_then(|value| Vec::<String>::try_from(value.try_clone().ok()?).ok())
        .and_then(|uris| uris.into_iter().next());
    Ok(uri.and_then(|uri| file_path(&uri)))
}

/// A local path from a `file://` URI, with percent escapes decoded.
pub fn file_path(uri: &str) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let encoded = uri.strip_prefix("file://")?;
    let encoded = encoded.strip_prefix("localhost").unwrap_or(encoded);
    let mut bytes = Vec::new();
    let mut input = encoded.bytes();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let high = (input.next()? as char).to_digit(16)?;
            let low = (input.next()? as char).to_digit(16)?;
            bytes.push((high * 16 + low) as u8);
        } else {
            bytes.push(byte);
        }
    }
    let path = PathBuf::from(std::ffi::OsString::from_vec(bytes));
    path.is_absolute().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopa_services::testing::{TestBus, network::*};
    use std::time::Duration;

    async fn state(service: &NetworkService) -> NetworkState {
        service.snapshot().await.unwrap()
    }

    #[test]
    fn portal_uris_become_paths() {
        assert_eq!(
            file_path("file:///home/me/My%20CA.pem"),
            Some(PathBuf::from("/home/me/My CA.pem"))
        );
        assert_eq!(file_path("https://example.com/x"), None);
        assert_eq!(file_path("file://relative"), None);
    }

    #[tokio::test]
    async fn the_page_is_unavailable_without_network_manager_and_recovers() {
        let bus = TestBus::new();
        let root = tempfile::tempdir().unwrap();
        let service = NetworkService::with(bus.connect().await, root.path().into(), "nmcli".into());
        assert!(matches!(
            service.snapshot().await,
            Err(ServiceError::Unavailable(_))
        ));
        let mut updates = service.updates();
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(3), updates.next()).await,
            Ok(Some(Err(ServiceError::Unavailable(_))))
        ));
        let _nm = FakeNetworkManager::serve(&bus).await;
        let state = state(&service).await;
        assert!(state.agent, "the agent registers with NetworkManager");
        assert_eq!(state.overview.devices.len(), 2);
    }

    #[tokio::test]
    async fn each_page_action_reaches_network_manager() {
        let bus = TestBus::new();
        let nm = FakeNetworkManager::serve(&bus).await;
        let root = tempfile::tempdir().unwrap();
        // A stand-in for nmcli that records its arguments.
        let nmcli = root.path().join("nmcli");
        let record = root.path().join("nmcli.args");
        std::fs::write(
            &nmcli,
            format!("#!/bin/sh\necho \"$@\" > {}\n", record.display()),
        )
        .unwrap();
        std::fs::set_permissions(&nmcli, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
        let service = NetworkService::with(bus.connect().await, root.path().into(), nmcli);
        let overview = state(&service).await.overview;
        let apply = async |action| service.apply(action).await.map(|state| state.unwrap());

        let off = apply(NetworkAction::WifiEnabled(false)).await.unwrap();
        assert!(!off.overview.wireless_enabled);
        apply(NetworkAction::WifiEnabled(true)).await.unwrap();

        // Saved and open networks connect directly; a secured one needs a form.
        let home = overview.access_points[0].clone();
        apply(NetworkAction::Connect(home)).await.unwrap();
        assert!(nm.calls().contains(&format!("Activate {HOME_PROFILE}")));
        let eduroam = overview.access_points[1].clone();
        assert!(matches!(
            apply(NetworkAction::Connect(eduroam)).await,
            Err(ServiceError::Failed(_))
        ));
        let cafe = overview.access_points[2].clone();
        apply(NetworkAction::Connect(cafe)).await.unwrap();
        assert!(nm.calls().contains(&"AddAndActivate Cafe".to_string()));

        // WPA-Enterprise with a CA certificate.
        let ca = root.path().join("ca.pem");
        std::fs::write(&ca, "certificate").unwrap();
        let joined = apply(NetworkAction::Join {
            ssid: b"eduroam".to_vec(),
            security: WifiSecurity::Enterprise(profiles::Enterprise {
                method: Some(profiles::EapMethod::Ttls),
                phase2: Some(profiles::Phase2::Pap),
                identity: "me@example.edu".into(),
                ca_certificate: Some(ca),
                password: "typed".into(),
                ..Default::default()
            }),
            hidden: false,
            device: WIFI.into(),
        })
        .await
        .unwrap();
        let eduroam = nm.profile("eduroam").unwrap();
        assert_eq!(
            <&str>::try_from(&eduroam["802-1x"]["phase2-auth"]).unwrap(),
            "pap"
        );
        assert!(
            joined
                .overview
                .connections
                .iter()
                .any(|c| c.id == "eduroam" && c.active.is_some())
        );

        // Forget, VPN connect and disconnect, settings changes.
        let saved = Target::from(overview.connection(HOME_PROFILE).unwrap());
        apply(NetworkAction::Forget(saved)).await.unwrap();
        let vpn = Target::from(overview.connection(VPN).unwrap());
        let connected = apply(NetworkAction::Activate(vpn.clone())).await.unwrap();
        let active = connected
            .overview
            .connection(VPN)
            .unwrap()
            .active
            .clone()
            .unwrap()
            .0;
        apply(NetworkAction::Deactivate(active)).await.unwrap();
        let wired = Target::from(overview.connection(WIRED).unwrap());
        apply(NetworkAction::Change(
            wired.clone(),
            Change::Metered(manager::Metered::Yes),
        ))
        .await
        .unwrap();
        assert!(matches!(
            apply(NetworkAction::Change(
                wired,
                Change::Ip("ipv4", manager::IpConfig { method: "manual".into(), ..Default::default() })
            ))
            .await,
            Err(ServiceError::Failed(message)) if message.contains("address")
        ));

        // VPN files: WireGuard is parsed here, others go to nmcli.
        let conf = root.path().join("office.conf");
        std::fs::write(
            &conf,
            "[Interface]\nPrivateKey = yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=\nAddress = 10.0.0.2/24\n[Peer]\nPublicKey = xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=\nAllowedIPs = 0.0.0.0/0\n",
        )
        .unwrap();
        apply(NetworkAction::Import(conf, None)).await.unwrap();
        assert!(nm.profile("office").is_some());
        let ovpn = root.path().join("work.ovpn");
        std::fs::write(&ovpn, "client\nremote vpn.example.com\n").unwrap();
        assert!(matches!(
            apply(NetworkAction::Import(ovpn.clone(), None)).await,
            Err(ServiceError::Failed(_))
        ));
        apply(NetworkAction::Import(ovpn.clone(), Some("openvpn".into())))
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&record).unwrap().trim(),
            format!("connection import type openvpn file {}", ovpn.display())
        );

        // A hotspot.
        apply(NetworkAction::Hotspot {
            ssid: "Share".into(),
            password: "share this".into(),
            device: WIFI.into(),
        })
        .await
        .unwrap();
        assert!(nm.calls().contains(&"AddAndActivate Hotspot".to_string()));
    }

    #[tokio::test]
    async fn network_manager_questions_reach_the_page_and_its_answer_goes_back() {
        let bus = TestBus::new();
        let nm = FakeNetworkManager::serve(&bus).await;
        let root = tempfile::tempdir().unwrap();
        let service = NetworkService::with(bus.connect().await, root.path().into(), "nmcli".into());
        assert!(state(&service).await.agent);
        let mut updates = service.updates();
        let ask = nm.ask_agent("Office VPN", "vpn", &["password"], 0x1);
        let answer = async {
            let question = tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    if let Some(Ok(state)) = updates.next().await
                        && let Some(question) = state.question
                    {
                        return question;
                    }
                }
            })
            .await
            .unwrap();
            assert_eq!(question.connection, "Office VPN");
            assert_eq!(question.fields[0].key, "password");
            let after = service
                .apply(NetworkAction::Answer(HashMap::from([(
                    "password".into(),
                    "one-time".into(),
                )])))
                .await
                .unwrap()
                .unwrap();
            assert!(after.question.is_none());
        };
        let (secrets, ()) = tokio::join!(ask, answer);
        let secrets = secrets.unwrap();
        let values: HashMap<String, String> = secrets["vpn"]["secrets"]
            .try_clone()
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(values["password"], "one-time");
        // Nothing waits any more.
        assert!(matches!(
            service.apply(NetworkAction::Answer(HashMap::new())).await,
            Err(ServiceError::Failed(_))
        ));
        assert!(
            format!(
                "{:?}",
                NetworkAction::Answer(HashMap::from([("a".into(), "secret".into())]))
            ) == "secret"
        );
    }
}
