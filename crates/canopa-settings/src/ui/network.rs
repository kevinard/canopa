//! The Network page: Wi-Fi (switch, nearby networks, joining with
//! WPA-Enterprise, hidden networks), wired status, VPNs (connect, WireGuard,
//! import), a hotspot, and each saved connection's settings (auto-connect,
//! metered, IPv4/IPv6 and DNS, proxy). NetworkManager's questions for
//! secrets appear at the top. Changes apply at once.
use super::{Message, State, choice, focus, live_page::LivePage, settings_group, toggle};
use crate::{
    live::Event,
    network::{NetworkAction, NetworkService, NetworkState, choose_file},
};
use canopa_services::network::{
    manager::{
        AccessPoint, Change, Connection, ConnectionKind, DeviceKind, IpConfig, Metered, Proxy,
        Security, Target,
    },
    profiles::{EapMethod, Enterprise, Peer, Phase2, SecretStorage, WifiSecurity, WireGuard},
};
use iced::{
    Alignment, Element, Length, Task,
    widget::{column, container, row, text, text_input},
};
use std::{collections::HashMap, path::PathBuf};

pub(super) type NetworkPage = LivePage<NetworkService>;

pub(super) fn message(event: Event<NetworkService>) -> Message {
    Message::Network(event)
}

fn apply(action: NetworkAction) -> Message {
    message(Event::Apply(action))
}

/// A text field the page's forms hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Field {
    Ssid,
    Password,
    Identity,
    Anonymous,
    Ca,
    Domain,
    ClientCertificate,
    PrivateKey,
    KeyPassword,
    Ip4Address,
    Ip4Gateway,
    Ip4Dns,
    Ip6Address,
    Ip6Gateway,
    Ip6Dns,
    Proxy,
    WgName,
    WgPrivate,
    WgAddress,
    WgDns,
    WgPeer,
    WgEndpoint,
    WgAllowed,
    ImportPath,
    HotspotSsid,
    HotspotPassword,
}

/// A choice the page's forms hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pick {
    Security,
    Method,
    Phase2,
    Ip4,
    Ip6,
    Proxy,
    ImportKind,
}

#[derive(Clone, Debug)]
pub(super) enum Edit {
    Text(Field, String),
    Choose(Pick, String),
    Answer(String, String),
    Remember(bool),
    Join(Option<AccessPoint>),
    CloseJoin,
    SubmitJoin,
    Select(String),
    ApplyIp(&'static str),
    ApplyProxy,
    WireGuard(bool),
    SubmitWireGuard,
    SubmitImport,
    StartHotspot,
    SubmitAnswer,
    ChooseFile(Field),
    FileChosen(Field, Option<PathBuf>),
}

const SECURITY: [&str; 4] = [
    "Open",
    "WPA/WPA2 Personal",
    "WPA3 Personal",
    "WPA-Enterprise",
];
const METHODS: [(&str, EapMethod); 3] = [
    ("PEAP", EapMethod::Peap),
    ("TTLS", EapMethod::Ttls),
    ("TLS (certificate)", EapMethod::Tls),
];
const PHASE2: [(&str, Phase2); 5] = [
    ("MSCHAPv2", Phase2::Mschapv2),
    ("PAP", Phase2::Pap),
    ("GTC", Phase2::Gtc),
    ("MSCHAP", Phase2::Mschap),
    ("CHAP", Phase2::Chap),
];
const IP4: [(&str, &str); 3] = [
    ("Automatic (DHCP)", "auto"),
    ("Manual", "manual"),
    ("Disabled", "disabled"),
];
const IP6: [(&str, &str); 3] = [
    ("Automatic", "auto"),
    ("Manual", "manual"),
    ("Disabled", "disabled"),
];
const PROXY: [&str; 3] = ["No proxy", "Automatic (configuration URL)", "Manual"];
const METERED: [(&str, Metered); 3] = [
    ("Automatic", Metered::Automatic),
    ("Metered", Metered::Yes),
    ("Not metered", Metered::No),
];

/// The page's unsaved form input. Secrets typed here go to NetworkManager
/// and are cleared when a form closes.
#[derive(Default)]
pub(super) struct NetworkForms {
    text: HashMap<Field, String>,
    security: String,
    method: String,
    phase2: String,
    ip4: String,
    ip6: String,
    proxy: String,
    import_kind: String,
    remember: bool,
    /// The join form: `Some(None)` for a hidden network.
    join: Option<Option<AccessPoint>>,
    selected: Option<String>,
    wireguard: bool,
    answers: HashMap<String, String>,
}

impl NetworkForms {
    fn get(&self, field: Field) -> &str {
        self.text.get(&field).map_or("", String::as_str)
    }
    fn set(&mut self, field: Field, value: &str) {
        self.text.insert(field, value.to_owned());
    }
    fn clear_secrets(&mut self) {
        for field in [Field::Password, Field::KeyPassword, Field::WgPrivate] {
            self.text.remove(&field);
        }
        self.answers.clear();
    }
}

fn list(text: &str) -> Vec<String> {
    text.split([',', ' '])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

fn path(text: &str) -> Option<PathBuf> {
    let text = text.trim();
    (!text.is_empty()).then(|| PathBuf::from(text))
}

fn muted<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label).size(14).style(canopa_ui::muted).into()
}

/// A button with a focus id of its own, for rows with repeated labels.
fn row_button<'a>(id: String, label: &'a str, message: Option<Message>) -> Element<'a, Message> {
    canopa_ui::accessibility::semantic(
        canopa_ui::accessibility::Node::button(label)
            .key(id.clone())
            .disabled(message.is_none())
            .on_click(message.clone()),
        focus::control(
            id,
            message.clone(),
            None,
            None,
            false,
            canopa_ui::action(text(label), message, canopa_ui::Action::Normal, false)
                .padding([6, 12]),
        ),
    )
}

fn secret<'a>(
    id: &str,
    label: &'a str,
    value: &str,
    change: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    // The secret itself never enters the accessibility tree: only its length.
    canopa_ui::accessibility::semantic(
        canopa_ui::accessibility::Node::new(canopa_ui::accessibility::Role::PasswordInput)
            .label(label)
            .value("•".repeat(value.chars().count()))
            .key(format!("network-{id}-field")),
        column![
            text(label),
            text_input("", value)
                .id(format!("network-{id}"))
                .secure(true)
                .on_input(change)
                .padding(10)
        ]
        .spacing(6)
        .width(Length::Fill),
    )
}

fn edit(edit: Edit) -> Message {
    Message::NetworkForm(edit)
}

fn text_field<'a>(forms: &NetworkForms, field: Field, label: &'a str) -> Element<'a, Message> {
    super::field(
        format!("network-{field:?}").to_lowercase(),
        label,
        forms.get(field),
        move |value| edit(Edit::Text(field, value)),
    )
}

fn security_label(security: Security) -> &'static str {
    match security {
        Security::Open => "open",
        Security::Personal => "secured",
        Security::Enterprise => "enterprise",
    }
}

fn kind_label(kind: &ConnectionKind) -> String {
    match kind {
        ConnectionKind::Wifi { hotspot: true, .. } => "hotspot".into(),
        ConnectionKind::Wifi { .. } => "Wi-Fi".into(),
        ConnectionKind::Ethernet => "wired".into(),
        ConnectionKind::WireGuard => "WireGuard".into(),
        ConnectionKind::Vpn(service) => service.rsplit('.').next().unwrap_or("VPN").to_owned(),
        ConnectionKind::Other(kind) => kind.clone(),
    }
}

fn connection_label(connection: &Connection) -> String {
    format!("{} ({})", connection.id, kind_label(&connection.kind))
}

impl State {
    pub(super) fn network_edit(&mut self, change: Edit) -> Task<Message> {
        let forms = &mut self.network_forms;
        let current = self.network.live.state().cloned();
        let wifi_device = current.as_ref().and_then(|state| {
            state
                .overview
                .devices
                .iter()
                .find(|device| device.kind == DeviceKind::Wifi)
                .map(|device| device.path.clone())
        });
        let send = |action| Task::done(apply(action));
        match change {
            Edit::Text(field, value) => {
                forms.text.insert(field, value.chars().take(1024).collect());
            }
            Edit::Choose(pick, value) => {
                let target = match pick {
                    Pick::Security => &mut forms.security,
                    Pick::Method => &mut forms.method,
                    Pick::Phase2 => &mut forms.phase2,
                    Pick::Ip4 => &mut forms.ip4,
                    Pick::Ip6 => &mut forms.ip6,
                    Pick::Proxy => &mut forms.proxy,
                    Pick::ImportKind => &mut forms.import_kind,
                };
                *target = value;
            }
            Edit::Answer(key, value) => {
                forms.answers.insert(key, value);
            }
            Edit::Remember(value) => forms.remember = value,
            Edit::Join(ap) => {
                forms.clear_secrets();
                forms.remember = true;
                forms.method = METHODS[0].0.into();
                forms.phase2 = PHASE2[0].0.into();
                forms.security = match ap.as_ref().map(|ap| ap.security) {
                    Some(Security::Enterprise) => SECURITY[3],
                    Some(Security::Open) => SECURITY[0],
                    _ => SECURITY[1],
                }
                .into();
                forms.set(
                    Field::Ssid,
                    &ap.as_ref().map(AccessPoint::name).unwrap_or_default(),
                );
                forms.join = Some(ap);
            }
            Edit::CloseJoin => {
                forms.join = None;
                forms.clear_secrets();
            }
            Edit::SubmitJoin => {
                let (Some(join), Some(device)) = (forms.join.clone(), wifi_device) else {
                    return Task::none();
                };
                let storage = if forms.remember {
                    SecretStorage::System
                } else {
                    SecretStorage::Ask
                };
                let security = match forms.security.as_str() {
                    "Open" => WifiSecurity::Open,
                    "WPA-Enterprise" => WifiSecurity::Enterprise(Enterprise {
                        method: METHODS
                            .iter()
                            .find(|(label, _)| *label == forms.method)
                            .map(|(_, method)| *method),
                        phase2: PHASE2
                            .iter()
                            .find(|(label, _)| *label == forms.phase2)
                            .map(|(_, phase2)| *phase2),
                        identity: forms.get(Field::Identity).into(),
                        anonymous_identity: forms.get(Field::Anonymous).into(),
                        ca_certificate: path(forms.get(Field::Ca)),
                        domain: forms.get(Field::Domain).into(),
                        password: forms.get(Field::Password).into(),
                        client_certificate: path(forms.get(Field::ClientCertificate)),
                        private_key: path(forms.get(Field::PrivateKey)),
                        private_key_password: forms.get(Field::KeyPassword).into(),
                        storage,
                    }),
                    other => WifiSecurity::Personal {
                        password: forms.get(Field::Password).into(),
                        sae: other == "WPA3 Personal",
                        storage,
                    },
                };
                let ssid = match &join {
                    Some(ap) => ap.ssid.clone(),
                    None => forms.get(Field::Ssid).trim().as_bytes().to_vec(),
                };
                forms.join = None;
                forms.clear_secrets();
                return send(NetworkAction::Join {
                    ssid,
                    security,
                    hidden: join.is_none(),
                    device,
                });
            }
            Edit::Select(label) => {
                let Some(connection) = current.as_ref().and_then(|state| {
                    state
                        .overview
                        .connections
                        .iter()
                        .find(|connection| connection_label(connection) == label)
                        .cloned()
                }) else {
                    return Task::none();
                };
                let method = |options: &[(&str, &str)], method: &str| {
                    options
                        .iter()
                        .find(|(_, id)| *id == method || (*id == "disabled" && method == "ignore"))
                        .map_or(options[0].0, |(label, _)| label)
                        .to_owned()
                };
                forms.ip4 = method(&IP4, &connection.ipv4.method);
                forms.ip6 = method(&IP6, &connection.ipv6.method);
                for (address, gateway, dns, ip) in [
                    (
                        Field::Ip4Address,
                        Field::Ip4Gateway,
                        Field::Ip4Dns,
                        &connection.ipv4,
                    ),
                    (
                        Field::Ip6Address,
                        Field::Ip6Gateway,
                        Field::Ip6Dns,
                        &connection.ipv6,
                    ),
                ] {
                    forms.set(address, &ip.addresses.join(", "));
                    forms.set(gateway, ip.gateway.as_deref().unwrap_or(""));
                    forms.set(dns, &ip.dns.join(", "));
                }
                let (proxy, value) = match &connection.proxy {
                    Proxy::None => (PROXY[0], String::new()),
                    Proxy::Automatic { url } => (PROXY[1], url.clone()),
                    Proxy::Manual { server } => (PROXY[2], server.clone()),
                };
                forms.proxy = proxy.into();
                forms.set(Field::Proxy, &value);
                forms.selected = Some(connection.path);
            }
            Edit::ApplyIp(family) => {
                let Some(target) = self.selected_target() else {
                    return Task::none();
                };
                let forms = &self.network_forms;
                let (options, choice, fields) = if family == "ipv4" {
                    (
                        &IP4[..],
                        &forms.ip4,
                        [Field::Ip4Address, Field::Ip4Gateway, Field::Ip4Dns],
                    )
                } else {
                    (
                        &IP6[..],
                        &forms.ip6,
                        [Field::Ip6Address, Field::Ip6Gateway, Field::Ip6Dns],
                    )
                };
                let method = options
                    .iter()
                    .find(|(label, _)| label == choice)
                    .map_or("auto", |(_, id)| id);
                let config = IpConfig {
                    method: method.into(),
                    addresses: list(forms.get(fields[0])),
                    gateway: Some(forms.get(fields[1]).trim().to_owned()).filter(|g| !g.is_empty()),
                    dns: list(forms.get(fields[2])),
                };
                return send(NetworkAction::Change(target, Change::Ip(family, config)));
            }
            Edit::ApplyProxy => {
                let Some(target) = self.selected_target() else {
                    return Task::none();
                };
                let forms = &self.network_forms;
                let value = forms.get(Field::Proxy).trim().to_owned();
                let proxy = match forms.proxy.as_str() {
                    "Manual" => Proxy::Manual { server: value },
                    "Automatic (configuration URL)" => Proxy::Automatic { url: value },
                    _ => Proxy::None,
                };
                return send(NetworkAction::Change(target, Change::Proxy(proxy)));
            }
            Edit::WireGuard(open) => {
                forms.wireguard = open;
                if !open {
                    forms.clear_secrets();
                }
            }
            Edit::SubmitWireGuard => {
                let tunnel = WireGuard {
                    name: forms.get(Field::WgName).trim().into(),
                    private_key: forms.get(Field::WgPrivate).trim().into(),
                    addresses: list(forms.get(Field::WgAddress)),
                    dns: list(forms.get(Field::WgDns)),
                    peers: vec![Peer {
                        public_key: forms.get(Field::WgPeer).trim().into(),
                        endpoint: forms.get(Field::WgEndpoint).trim().into(),
                        allowed_ips: list(forms.get(Field::WgAllowed)),
                        ..Default::default()
                    }],
                    ..Default::default()
                };
                forms.wireguard = false;
                forms.clear_secrets();
                return send(NetworkAction::AddWireGuard(tunnel));
            }
            Edit::SubmitImport => {
                let Some(file) = path(forms.get(Field::ImportPath)) else {
                    return Task::none();
                };
                let kind = match forms.import_kind.as_str() {
                    "" | "Detect WireGuard" => None,
                    "WireGuard" => Some("wireguard".to_owned()),
                    plugin => Some(plugin.to_owned()),
                };
                return send(NetworkAction::Import(file, kind));
            }
            Edit::StartHotspot => {
                let Some(device) = wifi_device else {
                    return Task::none();
                };
                let action = NetworkAction::Hotspot {
                    ssid: forms.get(Field::HotspotSsid).trim().into(),
                    password: forms.get(Field::HotspotPassword).into(),
                    device,
                };
                forms.text.remove(&Field::HotspotPassword);
                return send(action);
            }
            Edit::SubmitAnswer => {
                let answers = std::mem::take(&mut forms.answers);
                return send(NetworkAction::Answer(answers));
            }
            Edit::ChooseFile(field) => {
                let title = match field {
                    Field::Ca => "Choose the CA certificate",
                    Field::ClientCertificate => "Choose your certificate",
                    Field::PrivateKey => "Choose your private key",
                    _ => "Choose a VPN file",
                };
                return Task::perform(
                    async move { choose_file(title).await.ok().flatten() },
                    move |file| edit(Edit::FileChosen(field, file)),
                );
            }
            Edit::FileChosen(field, file) => {
                if let Some(file) = file.and_then(|file| file.to_str().map(str::to_owned)) {
                    forms.set(field, &file);
                }
            }
        }
        Task::none()
    }

    fn selected_target(&self) -> Option<Target> {
        let path = self.network_forms.selected.as_ref()?;
        let state = self.network.live.state()?;
        state.overview.connection(path).map(Target::from)
    }

    fn question_section<'a>(&'a self, state: &'a NetworkState) -> Option<Element<'a, Message>> {
        let question = state.question.as_ref()?;
        let forms = &self.network_forms;
        let mut body = column![text(if question.retry {
            format!(
                "The previous answer for {} was not accepted. Try again.",
                question.connection
            )
        } else {
            format!(
                "NetworkManager needs this to connect {}.",
                question.connection
            )
        })]
        .spacing(10);
        for field in &question.fields {
            let key = field.key.clone();
            body = body.push(secret(
                &format!("answer-{}", field.key),
                &field.label,
                forms.answers.get(&field.key).map_or("", String::as_str),
                move |value| edit(Edit::Answer(key.clone(), value)),
            ));
        }
        body = body
            .push(
                row![
                    row_button(
                        "network-answer".into(),
                        "Connect",
                        Some(edit(Edit::SubmitAnswer))
                    ),
                    row_button(
                        "network-answer-cancel".into(),
                        "Cancel",
                        Some(apply(NetworkAction::Cancel))
                    ),
                ]
                .spacing(10),
            )
            .push(muted(
                "Settings passes this to NetworkManager and does not keep it.",
            ))
            .push(self.network.control_status("secret", message));
        Some(settings_group("Sign-in needed", body))
    }

    fn join_form(&self) -> Option<Element<'_, Message>> {
        let join = self.network_forms.join.as_ref()?;
        let forms = &self.network_forms;
        let mut body = column![].spacing(10);
        if join.is_none() {
            body = body.push(text_field(forms, Field::Ssid, "Network name"));
        } else {
            body = body.push(text(format!("Join {}", forms.get(Field::Ssid))));
        }
        body = body.push(
            column![
                text("Security"),
                choice(
                    "network-security",
                    SECURITY.iter().map(|s| s.to_string()).collect(),
                    Some(forms.security.clone()),
                    |value| edit(Edit::Choose(Pick::Security, value)),
                )
            ]
            .spacing(6),
        );
        let password = |label| {
            secret("password", label, forms.get(Field::Password), |value| {
                edit(Edit::Text(Field::Password, value))
            })
        };
        let file = |field: Field, label: &'static str| -> Element<'_, Message> {
            row![
                text_field(forms, field, label),
                row_button(
                    format!("network-choose-{field:?}").to_lowercase(),
                    "Choose…",
                    Some(edit(Edit::ChooseFile(field))),
                ),
            ]
            .spacing(10)
            .align_y(Alignment::End)
            .into()
        };
        match forms.security.as_str() {
            "Open" => {}
            "WPA-Enterprise" => {
                body = body.push(
                    column![
                        text("Authentication"),
                        choice(
                            "network-eap",
                            METHODS.iter().map(|(label, _)| label.to_string()).collect(),
                            Some(forms.method.clone()),
                            |value| edit(Edit::Choose(Pick::Method, value)),
                        )
                    ]
                    .spacing(6),
                );
                let tls = forms.method == METHODS[2].0;
                if !tls {
                    body = body.push(
                        column![
                            text("Inner authentication"),
                            choice(
                                "network-phase2",
                                PHASE2.iter().map(|(label, _)| label.to_string()).collect(),
                                Some(forms.phase2.clone()),
                                |value| edit(Edit::Choose(Pick::Phase2, value)),
                            )
                        ]
                        .spacing(6),
                    );
                }
                body = body
                    .push(text_field(forms, Field::Identity, "Identity (user name)"))
                    .push(text_field(
                        forms,
                        Field::Anonymous,
                        "Anonymous identity (optional)",
                    ))
                    .push(file(
                        Field::Ca,
                        "CA certificate (empty: system certificates)",
                    ))
                    .push(text_field(
                        forms,
                        Field::Domain,
                        "Server domain (recommended)",
                    ));
                if tls {
                    body = body
                        .push(file(Field::ClientCertificate, "Your certificate"))
                        .push(file(Field::PrivateKey, "Your private key"))
                        .push(secret(
                            "key-password",
                            "Private key password",
                            forms.get(Field::KeyPassword),
                            |value| edit(Edit::Text(Field::KeyPassword, value)),
                        ));
                } else {
                    body = body.push(password("Password"));
                }
            }
            _ => body = body.push(password("Wi-Fi password")),
        }
        if forms.security != "Open" {
            body = body.push(toggle(
                "network-remember",
                "Remember the password (NetworkManager stores it)",
                forms.remember,
                edit(Edit::Remember(!forms.remember)),
            ));
        }
        body = body
            .push(
                row![
                    row_button("network-join".into(), "Join", Some(edit(Edit::SubmitJoin))),
                    row_button(
                        "network-join-cancel".into(),
                        "Cancel",
                        Some(edit(Edit::CloseJoin))
                    ),
                ]
                .spacing(10),
            )
            .push(self.network.control_status("wifi-join", message));
        Some(
            container(body)
                .padding(12)
                .width(Length::Fill)
                .style(canopa_ui::surface)
                .into(),
        )
    }

    fn wifi_section<'a>(&'a self, state: &'a NetworkState) -> Element<'a, Message> {
        let page = &self.network;
        let overview = &state.overview;
        if !overview
            .devices
            .iter()
            .any(|device| device.kind == DeviceKind::Wifi)
        {
            return settings_group("Wi-Fi", muted("No Wi-Fi device."));
        }
        let enabled = match page.live.pending("wifi-enabled") {
            Some(NetworkAction::WifiEnabled(value)) => *value,
            _ => overview.wireless_enabled,
        };
        let mut body = column![
            toggle(
                "network-wifi",
                "Wi-Fi",
                enabled,
                apply(NetworkAction::WifiEnabled(!enabled))
            ),
            page.control_status("wifi-enabled", message),
        ]
        .spacing(10);
        if !overview.wireless_hardware_enabled {
            body = body.push(muted(
                "A hardware switch or airplane-mode key turned Wi-Fi off.",
            ));
        }
        if let Some(form) = self.join_form() {
            body = body.push(form);
        }
        if enabled {
            for ap in overview.access_points.iter().take(16) {
                let saved = overview.saved_for(ap);
                let active = saved.and_then(|connection| connection.active.clone());
                let mut description = format!("{} · {}%", security_label(ap.security), ap.strength);
                if active.is_some() {
                    description = format!("Connected · {description}");
                } else if saved.is_some() {
                    description = format!("Saved · {description}");
                }
                let control = format!("wifi-{}", ap.name());
                let action = match (&active, saved, ap.security) {
                    (Some((path, _)), ..) => {
                        ("Disconnect", apply(NetworkAction::Deactivate(path.clone())))
                    }
                    (None, Some(_), _) | (None, None, Security::Open) => {
                        ("Connect", apply(NetworkAction::Connect(ap.clone())))
                    }
                    (None, None, _) => ("Connect…", edit(Edit::Join(Some(ap.clone())))),
                };
                body = body.push(
                    column![
                        row![
                            column![text(ap.name()), muted(description)]
                                .spacing(2)
                                .width(Length::Fill),
                            row_button(format!("network-{control}"), action.0, Some(action.1),),
                        ]
                        .spacing(10)
                        .align_y(Alignment::Center),
                        page.control_status(&control, message),
                    ]
                    .spacing(4),
                );
            }
            if overview.access_points.is_empty() {
                body = body.push(muted("No networks found yet."));
            }
            body = body.push(row_button(
                "network-hidden".into(),
                "Join a hidden network…",
                Some(edit(Edit::Join(None))),
            ));
        }
        settings_group("Wi-Fi", body)
    }

    fn wired_section<'a>(&'a self, state: &'a NetworkState) -> Element<'a, Message> {
        let mut body = column![].spacing(8);
        let mut any = false;
        for device in state
            .overview
            .devices
            .iter()
            .filter(|device| device.kind == DeviceKind::Ethernet)
        {
            any = true;
            let status = match (device.connected(), device.state) {
                (true, _) => {
                    let name = device
                        .connection
                        .as_deref()
                        .and_then(|path| state.overview.connection(path))
                        .map(|connection| connection.id.clone())
                        .unwrap_or_default();
                    format!("Connected · {name} · {}", device.addresses.join(", "))
                }
                (false, 20) => "Cable unplugged".into(),
                (false, 40..=90) => "Connecting…".into(),
                _ => "Disconnected".into(),
            };
            body = body.push(column![text(&device.interface), muted(status)].spacing(2));
        }
        if !any {
            body = body.push(muted("No wired network device."));
        }
        body = body.push(muted(
            "Change a wired connection's addresses and DNS under Connection settings.",
        ));
        settings_group("Wired", body)
    }

    fn vpn_section<'a>(&'a self, state: &'a NetworkState) -> Element<'a, Message> {
        let page = &self.network;
        let forms = &self.network_forms;
        let mut body = column![].spacing(10);
        let vpns: Vec<_> = state
            .overview
            .connections
            .iter()
            .filter(|connection| connection.kind.is_vpn())
            .collect();
        for connection in &vpns {
            let target = Target::from(*connection);
            let control = format!("connection-{}", connection.uuid);
            let (label, action) = match &connection.active {
                Some((path, _)) => ("Disconnect", NetworkAction::Deactivate(path.clone())),
                None => ("Connect", NetworkAction::Activate(target.clone())),
            };
            let status = if connection.active.is_some() {
                "Connected"
            } else {
                "Not connected"
            };
            let active_control = connection
                .active
                .as_ref()
                .map(|(path, _)| format!("active-{path}"));
            body = body.push(
                column![
                    row![
                        column![
                            text(&connection.id),
                            muted(format!("{} · {status}", kind_label(&connection.kind)))
                        ]
                        .spacing(2)
                        .width(Length::Fill),
                        row_button(
                            format!("network-vpn-{}", connection.uuid),
                            label,
                            Some(apply(action))
                        ),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                    page.control_status(&control, message),
                    page.control_status(active_control.as_deref().unwrap_or("none"), message),
                ]
                .spacing(4),
            );
        }
        if vpns.is_empty() {
            body = body.push(muted("No VPN connections."));
        }
        if forms.wireguard {
            body = body.push(
                container(
                    column![
                        text_field(forms, Field::WgName, "Name"),
                        secret(
                            "wg-private",
                            "Your private key",
                            forms.get(Field::WgPrivate),
                            |value| { edit(Edit::Text(Field::WgPrivate, value)) }
                        ),
                        text_field(
                            forms,
                            Field::WgAddress,
                            "Addresses (for example 10.0.0.2/24)"
                        ),
                        text_field(forms, Field::WgDns, "DNS servers (optional)"),
                        text_field(forms, Field::WgPeer, "Peer public key"),
                        text_field(forms, Field::WgEndpoint, "Peer endpoint (host:port)"),
                        text_field(
                            forms,
                            Field::WgAllowed,
                            "Allowed IPs (for example 0.0.0.0/0)"
                        ),
                        row![
                            row_button(
                                "network-wg-add".into(),
                                "Add",
                                Some(edit(Edit::SubmitWireGuard))
                            ),
                            row_button(
                                "network-wg-cancel".into(),
                                "Cancel",
                                Some(edit(Edit::WireGuard(false)))
                            ),
                        ]
                        .spacing(10),
                    ]
                    .spacing(10),
                )
                .padding(12)
                .width(Length::Fill)
                .style(canopa_ui::surface),
            );
        } else {
            body = body.push(row_button(
                "network-wg-open".into(),
                "Add a WireGuard tunnel…",
                Some(edit(Edit::WireGuard(true))),
            ));
        }
        let mut kinds = vec!["Detect WireGuard".to_owned(), "WireGuard".to_owned()];
        kinds.extend(
            state
                .overview
                .vpn_plugins
                .iter()
                .map(|plugin| plugin.name.clone()),
        );
        let selected = if forms.import_kind.is_empty() {
            kinds[0].clone()
        } else {
            forms.import_kind.clone()
        };
        body = body
            .push(text("Import a VPN file"))
            .push(
                row![
                    text_field(forms, Field::ImportPath, "File (.conf, .ovpn, …)"),
                    row_button(
                        "network-import-choose".into(),
                        "Choose…",
                        Some(edit(Edit::ChooseFile(Field::ImportPath))),
                    ),
                ]
                .spacing(10)
                .align_y(Alignment::End),
            )
            .push(
                row![
                    choice("network-import-kind", kinds, Some(selected), |value| {
                        edit(Edit::Choose(Pick::ImportKind, value))
                    }),
                    row_button("network-import".into(), "Import", Some(edit(Edit::SubmitImport))),
                ]
                .spacing(10),
            )
            .push(muted(if state.overview.vpn_plugins.is_empty() {
                "OpenVPN and IPsec files need their NetworkManager plugin, such as networkmanager-openvpn."
            } else {
                "Plugin VPN files are read by the installed NetworkManager plugin."
            }))
            .push(page.control_status("vpn-add", message));
        settings_group("VPN", body)
    }

    fn hotspot_section<'a>(&'a self, state: &'a NetworkState) -> Element<'a, Message> {
        let forms = &self.network_forms;
        let active = state.overview.connections.iter().find(|connection| {
            matches!(connection.kind, ConnectionKind::Wifi { hotspot: true, .. })
                && connection.active.is_some()
        });
        let body: Element<'_, Message> = if !state
            .overview
            .devices
            .iter()
            .any(|device| device.kind == DeviceKind::Wifi)
        {
            muted("A hotspot needs a Wi-Fi device.")
        } else if let Some(hotspot) = active {
            let path = hotspot
                .active
                .as_ref()
                .map(|(path, _)| path.clone())
                .unwrap_or_default();
            column![
                text(format!("Sharing as {}", hotspot.id)),
                row_button(
                    "network-hotspot-stop".into(),
                    "Stop the hotspot",
                    Some(apply(NetworkAction::Deactivate(path.clone()))),
                ),
                self.network
                    .control_status(&format!("active-{path}"), message),
            ]
            .spacing(10)
            .into()
        } else {
            column![
                text_field(forms, Field::HotspotSsid, "Network name"),
                secret(
                    "hotspot-password",
                    "Password (8 to 63 characters)",
                    forms.get(Field::HotspotPassword),
                    |value| { edit(Edit::Text(Field::HotspotPassword, value)) }
                ),
                row_button(
                    "network-hotspot".into(),
                    "Start the hotspot",
                    Some(edit(Edit::StartHotspot))
                ),
                muted("Wi-Fi disconnects from its network while the hotspot runs."),
                self.network.control_status("hotspot", message),
            ]
            .spacing(10)
            .into()
        };
        settings_group("Hotspot", body)
    }

    fn connection_section<'a>(&'a self, state: &'a NetworkState) -> Element<'a, Message> {
        let page = &self.network;
        let forms = &self.network_forms;
        let labels: Vec<String> = state
            .overview
            .connections
            .iter()
            .map(connection_label)
            .collect();
        if labels.is_empty() {
            return settings_group("Connection settings", muted("No saved connections."));
        }
        let selected = forms
            .selected
            .as_deref()
            .and_then(|path| state.overview.connection(path));
        let mut body = column![choice(
            "network-connection",
            labels,
            selected.map(connection_label),
            |value| edit(Edit::Select(value)),
        )]
        .spacing(10);
        let Some(connection) = selected else {
            return settings_group(
                "Connection settings",
                body.push(muted("Choose a saved connection.")),
            );
        };
        let target = Target::from(connection);
        let key = |kind: &str| format!("change-{}-{kind}", connection.uuid);
        let metered = METERED
            .iter()
            .find(|(_, value)| *value == connection.metered)
            .map(|(label, _)| label.to_string());
        let ip = |family: &'static str,
                  title: &'a str,
                  options: &[(&str, &str)],
                  current: &str,
                  fields: [Field; 3]|
         -> Element<'a, Message> {
            let manual = options
                .iter()
                .find(|(label, _)| *label == current)
                .is_some_and(|(_, id)| *id == "manual");
            let pick = if family == "ipv4" {
                Pick::Ip4
            } else {
                Pick::Ip6
            };
            let mut group = column![
                text(title),
                choice(
                    format!("network-{family}-method"),
                    options.iter().map(|(label, _)| label.to_string()).collect(),
                    Some(current.to_owned()),
                    move |value| edit(Edit::Choose(pick, value)),
                ),
            ]
            .spacing(8);
            if manual {
                group = group
                    .push(text_field(
                        forms,
                        fields[0],
                        "Addresses with prefix (for example 192.168.1.10/24)",
                    ))
                    .push(text_field(forms, fields[1], "Gateway"));
            }
            group
                .push(text_field(
                    forms,
                    fields[2],
                    "DNS servers (empty: from the network)",
                ))
                .push(row_button(
                    format!("network-{family}-apply"),
                    "Apply",
                    Some(edit(Edit::ApplyIp(family))),
                ))
                .push(page.control_status(&key(family), message))
                .into()
        };
        body = body
            .push(toggle(
                "network-autoconnect",
                "Connect automatically",
                connection.autoconnect,
                apply(NetworkAction::Change(
                    target.clone(),
                    Change::Autoconnect(!connection.autoconnect),
                )),
            ))
            .push(page.control_status(&key("autoconnect"), message))
            .push(
                column![
                    text("Metered connection (limits background downloads)"),
                    choice(
                        "network-metered",
                        METERED.iter().map(|(label, _)| label.to_string()).collect(),
                        metered,
                        {
                            let target = target.clone();
                            move |value| {
                                let metered = METERED
                                    .iter()
                                    .find(|(label, _)| *label == value)
                                    .map_or(Metered::Automatic, |(_, metered)| *metered);
                                apply(NetworkAction::Change(
                                    target.clone(),
                                    Change::Metered(metered),
                                ))
                            }
                        },
                    ),
                    page.control_status(&key("metered"), message),
                ]
                .spacing(6),
            );
        if !connection.kind.is_vpn() {
            body = body
                .push(ip(
                    "ipv4",
                    "IPv4",
                    &IP4,
                    &forms.ip4,
                    [Field::Ip4Address, Field::Ip4Gateway, Field::Ip4Dns],
                ))
                .push(ip(
                    "ipv6",
                    "IPv6",
                    &IP6,
                    &forms.ip6,
                    [Field::Ip6Address, Field::Ip6Gateway, Field::Ip6Dns],
                ));
        }
        let mut proxy = column![
            text("Proxy"),
            choice(
                "network-proxy",
                PROXY.iter().map(|s| s.to_string()).collect(),
                Some(forms.proxy.clone()),
                |value| { edit(Edit::Choose(Pick::Proxy, value)) }
            ),
        ]
        .spacing(8);
        match forms.proxy.as_str() {
            "Manual" => {
                proxy = proxy.push(text_field(forms, Field::Proxy, "Proxy server (host:port)"))
            }
            "Automatic (configuration URL)" => {
                proxy = proxy.push(text_field(forms, Field::Proxy, "Configuration URL (PAC)"))
            }
            _ => {}
        }
        body = body
            .push(
                proxy
                    .push(row_button(
                        "network-proxy-apply".into(),
                        "Apply",
                        Some(edit(Edit::ApplyProxy)),
                    ))
                    .push(muted(
                        "Applications that ask the system proxy (PacRunner, libproxy) use this.",
                    ))
                    .push(page.control_status(&key("proxy"), message)),
            )
            .push(row_button(
                "network-forget".into(),
                "Forget this connection",
                Some(apply(NetworkAction::Forget(target))),
            ))
            .push(page.control_status(&format!("connection-{}", connection.uuid), message));
        settings_group("Connection settings", body)
    }

    pub(super) fn network_view(&self) -> Element<'_, Message> {
        self.network.view(message, |state| {
            let mut body = column![].spacing(14);
            if let Some(question) = self.question_section(state) {
                body = body.push(question);
            }
            if !state.agent {
                body = body.push(muted(
                    "Settings could not offer NetworkManager its secret agent; connections that ask for a password each time cannot connect from here.",
                ));
            }
            body.push(self.wifi_section(state))
                .push(self.wired_section(state))
                .push(self.vpn_section(state))
                .push(self.hotspot_section(state))
                .push(self.connection_section(state))
                .into()
        })
    }
}
