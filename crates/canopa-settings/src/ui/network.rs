//! The Network page: Wi-Fi (switch, nearby networks, joining with
//! WPA-Enterprise, hidden networks), wired status, VPNs (connect, WireGuard,
//! import), a hotspot, and each saved connection's settings (auto-connect,
//! metered, IPv4/IPv6 and DNS, proxy). NetworkManager's questions for
//! secrets appear at the top. Changes apply at once.
use super::{Message, State, focus, live_page::LivePage, pick, settings_group, toggle};
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
use canopa_ui::direction;
use iced::{
    Alignment, Element, Length, Task,
    widget::{column, container, text, text_input},
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
    /// A choice by its identifier (see the constants below), never its label.
    Choose(Pick, String),
    Answer(String, String),
    Remember(bool),
    Join(Option<AccessPoint>),
    CloseJoin,
    SubmitJoin,
    /// A saved connection, by its D-Bus path.
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

// Choices are held by these stable identifiers and shown through their
// translated labels.
const SECURITY: [&str; 4] = ["open", "personal", "sae", "enterprise"];
const METHODS: [(&str, EapMethod); 3] = [
    ("peap", EapMethod::Peap),
    ("ttls", EapMethod::Ttls),
    ("tls", EapMethod::Tls),
];
const PHASE2: [(&str, Phase2); 5] = [
    ("MSCHAPv2", Phase2::Mschapv2),
    ("PAP", Phase2::Pap),
    ("GTC", Phase2::Gtc),
    ("MSCHAP", Phase2::Mschap),
    ("CHAP", Phase2::Chap),
];
/// NetworkManager's address methods.
const IP_METHODS: [&str; 3] = ["auto", "manual", "disabled"];
const PROXY: [&str; 3] = ["none", "auto", "manual"];
const METERED: [Metered; 3] = [Metered::Automatic, Metered::Yes, Metered::No];
/// The import choice that detects WireGuard files; plugins use their names.
const DETECT: &str = "";

fn security_choice(id: &str) -> String {
    match id {
        "open" => fl!("settings-network-security-open"),
        "sae" => fl!("settings-network-security-sae"),
        "enterprise" => fl!("settings-network-security-enterprise"),
        _ => fl!("settings-network-security-personal"),
    }
}

fn method_label(id: &str) -> String {
    match id {
        "peap" => "PEAP".into(),
        "ttls" => "TTLS".into(),
        _ => fl!("settings-network-eap-tls"),
    }
}

fn ip_label(family: &str, id: &str) -> String {
    match (family, id) {
        ("ipv4", "auto") => fl!("settings-network-ip-dhcp"),
        (_, "auto") => fl!("settings-network-ip-auto"),
        (_, "manual") => fl!("settings-network-ip-manual"),
        _ => fl!("settings-network-ip-disabled"),
    }
}

fn proxy_label(id: &str) -> String {
    match id {
        "auto" => fl!("settings-network-proxy-auto"),
        "manual" => fl!("settings-network-proxy-manual"),
        _ => fl!("settings-network-proxy-none"),
    }
}

fn metered_label(metered: Metered) -> String {
    match metered {
        Metered::Yes => fl!("settings-network-metered-yes"),
        Metered::No => fl!("settings-network-metered-no"),
        _ => fl!("settings-network-metered-auto"),
    }
}

/// Identifiers with their labels, for [`pick`].
fn options(ids: &[&str], label: impl Fn(&str) -> String) -> Vec<(String, String)> {
    ids.iter().map(|id| ((*id).to_owned(), label(id))).collect()
}

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
    text(label).size(13).style(canopa_ui::muted).into()
}

/// A button with a focus id of its own, for rows with repeated labels.
fn row_button<'a>(id: String, label: String, message: Option<Message>) -> Element<'a, Message> {
    canopa_ui::accessibility::semantic(
        canopa_ui::accessibility::Node::button(label.clone())
            .key(id.clone())
            .disabled(message.is_none())
            .on_click(message.clone()),
        focus::control(
            id,
            message.clone(),
            None,
            None,
            false,
            iced::widget::button(text(label))
                .on_press_maybe(message)
                .style(|theme, status| {
                    super::kit::button_style(theme, status, super::kit::Treatment::Secondary)
                })
                .padding([7, 14]),
        ),
    )
}

fn secret<'a>(
    id: &str,
    label: String,
    value: &str,
    change: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    // The secret itself never enters the accessibility tree: only its length.
    canopa_ui::accessibility::semantic(
        canopa_ui::accessibility::Node::new(canopa_ui::accessibility::Role::PasswordInput)
            .label(label.clone())
            .value("•".repeat(value.chars().count()))
            .key(format!("network-{id}-field")),
        column![
            text(label),
            text_input("", value)
                .id(format!("network-{id}"))
                .secure(true)
                .on_input(change)
                .align_x(direction::start())
                .padding([6, 12])
                .style(super::kit::text_input_style)
        ]
        .spacing(7)
        .width(Length::Fill),
    )
}

fn edit(edit: Edit) -> Message {
    Message::NetworkForm(edit)
}

fn text_field<'a>(forms: &NetworkForms, field: Field, label: String) -> Element<'a, Message> {
    super::field(
        format!("network-{field:?}").to_lowercase(),
        label,
        forms.get(field),
        move |value| edit(Edit::Text(field, value)),
    )
}

fn security_label(security: Security) -> String {
    match security {
        Security::Open => fl!("settings-network-ap-open"),
        Security::Personal => fl!("settings-network-ap-secured"),
        Security::Enterprise => fl!("settings-network-ap-enterprise"),
    }
}

fn kind_label(kind: &ConnectionKind) -> String {
    match kind {
        ConnectionKind::Wifi { hotspot: true, .. } => fl!("settings-network-kind-hotspot"),
        ConnectionKind::Wifi { .. } => "Wi-Fi".into(),
        ConnectionKind::Ethernet => fl!("settings-network-kind-wired"),
        ConnectionKind::WireGuard => "WireGuard".into(),
        ConnectionKind::Vpn(service) => service.rsplit('.').next().unwrap_or("VPN").to_owned(),
        ConnectionKind::Other(kind) => kind.clone(),
    }
}

fn connection_label(connection: &Connection) -> String {
    fl!(
        "settings-network-connection-option",
        name = connection.id.as_str(),
        kind = kind_label(&connection.kind)
    )
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
                    "open" => WifiSecurity::Open,
                    "enterprise" => WifiSecurity::Enterprise(Enterprise {
                        method: METHODS
                            .iter()
                            .find(|(id, _)| *id == forms.method)
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
                        sae: other == "sae",
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
            Edit::Select(path) => {
                let Some(connection) = current
                    .as_ref()
                    .and_then(|state| state.overview.connection(&path).cloned())
                else {
                    return Task::none();
                };
                let method = |method: &str| {
                    IP_METHODS
                        .iter()
                        .find(|id| **id == method || (**id == "disabled" && method == "ignore"))
                        .map_or(IP_METHODS[0], |id| id)
                        .to_owned()
                };
                forms.ip4 = method(&connection.ipv4.method);
                forms.ip6 = method(&connection.ipv6.method);
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
                let (choice, fields) = if family == "ipv4" {
                    (
                        &forms.ip4,
                        [Field::Ip4Address, Field::Ip4Gateway, Field::Ip4Dns],
                    )
                } else {
                    (
                        &forms.ip6,
                        [Field::Ip6Address, Field::Ip6Gateway, Field::Ip6Dns],
                    )
                };
                let method = IP_METHODS
                    .iter()
                    .find(|id| *id == choice)
                    .map_or("auto", |id| id);
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
                    "manual" => Proxy::Manual { server: value },
                    "auto" => Proxy::Automatic { url: value },
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
                    DETECT => None,
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
                    Field::Ca => fl!("settings-network-choose-ca"),
                    Field::ClientCertificate => fl!("settings-network-choose-certificate"),
                    Field::PrivateKey => fl!("settings-network-choose-key"),
                    _ => fl!("settings-network-choose-vpn"),
                };
                return Task::perform(
                    async move { choose_file(&title).await.ok().flatten() },
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
            fl!(
                "settings-network-question-retry",
                connection = question.connection.as_str()
            )
        } else {
            fl!(
                "settings-network-question",
                connection = question.connection.as_str()
            )
        })]
        .spacing(10);
        for field in &question.fields {
            let key = field.key.clone();
            body = body.push(secret(
                &format!("answer-{}", field.key),
                // NetworkManager's own label for the secret.
                field.label.clone(),
                forms.answers.get(&field.key).map_or("", String::as_str),
                move |value| edit(Edit::Answer(key.clone(), value)),
            ));
        }
        body = body
            .push(
                direction::row([
                    row_button(
                        "network-answer".into(),
                        fl!("settings-network-connect"),
                        Some(edit(Edit::SubmitAnswer)),
                    ),
                    row_button(
                        "network-answer-cancel".into(),
                        fl!("settings-cancel"),
                        Some(apply(NetworkAction::Cancel)),
                    ),
                ])
                .spacing(10),
            )
            .push(muted(fl!("settings-network-question-hint")))
            .push(self.network.control_status("secret", message));
        Some(settings_group(fl!("settings-network-question-title"), body))
    }

    fn join_form(&self) -> Option<Element<'_, Message>> {
        let join = self.network_forms.join.as_ref()?;
        let forms = &self.network_forms;
        let mut body = column![].spacing(10);
        if join.is_none() {
            body = body.push(text_field(forms, Field::Ssid, fl!("settings-network-name")));
        } else {
            body = body.push(text(fl!(
                "settings-network-join-named",
                network = forms.get(Field::Ssid)
            )));
        }
        body = body.push(
            column![
                text(fl!("settings-network-security")),
                pick(
                    "network-security",
                    options(&SECURITY, security_choice),
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
        let file = |field: Field, label: String| -> Element<'_, Message> {
            direction::row([
                text_field(forms, field, label),
                row_button(
                    format!("network-choose-{field:?}").to_lowercase(),
                    fl!("settings-network-choose"),
                    Some(edit(Edit::ChooseFile(field))),
                ),
            ])
            .spacing(10)
            .align_y(Alignment::End)
            .into()
        };
        match forms.security.as_str() {
            "open" => {}
            "enterprise" => {
                let methods: Vec<&str> = METHODS.iter().map(|(id, _)| *id).collect();
                body = body.push(
                    column![
                        text(fl!("settings-network-eap")),
                        pick(
                            "network-eap",
                            options(&methods, method_label),
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
                            text(fl!("settings-network-phase2")),
                            pick(
                                "network-phase2",
                                // Protocol names, the same in every language.
                                PHASE2
                                    .iter()
                                    .map(|(name, _)| ((*name).to_owned(), (*name).to_owned()))
                                    .collect(),
                                Some(forms.phase2.clone()),
                                |value| edit(Edit::Choose(Pick::Phase2, value)),
                            )
                        ]
                        .spacing(6),
                    );
                }
                body = body
                    .push(text_field(
                        forms,
                        Field::Identity,
                        fl!("settings-network-identity"),
                    ))
                    .push(text_field(
                        forms,
                        Field::Anonymous,
                        fl!("settings-network-anonymous"),
                    ))
                    .push(file(Field::Ca, fl!("settings-network-ca")))
                    .push(text_field(
                        forms,
                        Field::Domain,
                        fl!("settings-network-domain"),
                    ));
                if tls {
                    body = body
                        .push(file(
                            Field::ClientCertificate,
                            fl!("settings-network-certificate"),
                        ))
                        .push(file(Field::PrivateKey, fl!("settings-network-private-key")))
                        .push(secret(
                            "key-password",
                            fl!("settings-network-key-password"),
                            forms.get(Field::KeyPassword),
                            |value| edit(Edit::Text(Field::KeyPassword, value)),
                        ));
                } else {
                    body = body.push(password(fl!("settings-network-password")));
                }
            }
            _ => body = body.push(password(fl!("settings-network-wifi-password"))),
        }
        if forms.security != "open" {
            body = body.push(toggle(
                "network-remember",
                fl!("settings-network-remember"),
                forms.remember,
                edit(Edit::Remember(!forms.remember)),
            ));
        }
        body = body
            .push(
                direction::row([
                    row_button(
                        "network-join".into(),
                        fl!("settings-network-join"),
                        Some(edit(Edit::SubmitJoin)),
                    ),
                    row_button(
                        "network-join-cancel".into(),
                        fl!("settings-cancel"),
                        Some(edit(Edit::CloseJoin)),
                    ),
                ])
                .spacing(10),
            )
            .push(self.network.control_status("wifi-join", message));
        Some(
            container(body)
                .padding(12)
                .width(Length::Fill)
                .style(super::kit::note)
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
            return settings_group("Wi-Fi", muted(fl!("settings-network-no-wifi")));
        }
        let enabled = match page.live.pending("wifi-enabled") {
            Some(NetworkAction::WifiEnabled(value)) => *value,
            _ => overview.wireless_enabled,
        };
        let mut head = column![
            toggle(
                "network-wifi",
                "Wi-Fi",
                enabled,
                apply(NetworkAction::WifiEnabled(!enabled))
            ),
            page.control_status("wifi-enabled", message),
        ]
        .spacing(4);
        if !overview.wireless_hardware_enabled {
            head = head.push(muted(fl!("settings-network-wifi-hardware-off")));
        }
        let mut body: Vec<Element<'_, Message>> = vec![head.into()];
        if let Some(form) = self.join_form() {
            body.push(form);
        }
        if enabled {
            for ap in overview.access_points.iter().take(16) {
                let saved = overview.saved_for(ap);
                let active = saved.and_then(|connection| connection.active.clone());
                let description = fl!(
                    "settings-network-ap-description",
                    state = if active.is_some() {
                        "connected"
                    } else if saved.is_some() {
                        "saved"
                    } else {
                        "other"
                    },
                    security = security_label(ap.security),
                    strength = canopa_i18n::format::percent(f64::from(ap.strength))
                );
                let control = format!("wifi-{}", ap.name());
                let action = match (&active, saved, ap.security) {
                    (Some((path, _)), ..) => (
                        fl!("settings-network-disconnect"),
                        apply(NetworkAction::Deactivate(path.clone())),
                    ),
                    (None, Some(_), _) | (None, None, Security::Open) => (
                        fl!("settings-network-connect"),
                        apply(NetworkAction::Connect(ap.clone())),
                    ),
                    (None, None, _) => (
                        fl!("settings-network-connect-ask"),
                        edit(Edit::Join(Some(ap.clone()))),
                    ),
                };
                body.push(
                    column![
                        direction::row([
                            column![text(ap.name()), muted(description)]
                                .spacing(2)
                                .align_x(direction::start())
                                .width(Length::Fill)
                                .into(),
                            row_button(format!("network-{control}"), action.0, Some(action.1)),
                        ])
                        .spacing(10)
                        .align_y(Alignment::Center),
                        page.control_status(&control, message),
                    ]
                    .spacing(4)
                    .into(),
                );
            }
            if overview.access_points.is_empty() {
                body.push(super::kit::empty(fl!("settings-network-none-found")));
            }
            body.push(row_button(
                "network-hidden".into(),
                fl!("settings-network-join-hidden"),
                Some(edit(Edit::Join(None))),
            ));
        }
        settings_group("Wi-Fi", super::kit::rows(body))
    }

    fn wired_section<'a>(&'a self, state: &'a NetworkState) -> Element<'a, Message> {
        let mut body: Vec<Element<'_, Message>> = Vec::new();
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
                    fl!(
                        "settings-network-wired-connected",
                        name = name,
                        addresses = device.addresses.join(", ")
                    )
                }
                (false, 20) => fl!("settings-network-wired-unplugged"),
                (false, 40..=90) => fl!("settings-network-connecting"),
                _ => fl!("settings-network-disconnected"),
            };
            body.push(
                column![text(&device.interface), muted(status)]
                    .spacing(2)
                    .into(),
            );
        }
        if !any {
            body.push(super::kit::empty(fl!("settings-network-no-wired")));
        }
        body.push(muted(fl!("settings-network-wired-hint")));
        settings_group(fl!("settings-network-wired"), super::kit::rows(body))
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
                Some((path, _)) => (
                    fl!("settings-network-disconnect"),
                    NetworkAction::Deactivate(path.clone()),
                ),
                None => (
                    fl!("settings-network-connect"),
                    NetworkAction::Activate(target.clone()),
                ),
            };
            let active_control = connection
                .active
                .as_ref()
                .map(|(path, _)| format!("active-{path}"));
            body = body.push(
                column![
                    direction::row([
                        column![
                            text(&connection.id),
                            muted(fl!(
                                "settings-network-vpn-status",
                                kind = kind_label(&connection.kind),
                                connected = if connection.active.is_some() {
                                    "yes"
                                } else {
                                    "no"
                                }
                            ))
                        ]
                        .spacing(2)
                        .align_x(direction::start())
                        .width(Length::Fill)
                        .into(),
                        row_button(
                            format!("network-vpn-{}", connection.uuid),
                            label,
                            Some(apply(action)),
                        ),
                    ])
                    .spacing(10)
                    .align_y(Alignment::Center),
                    page.control_status(&control, message),
                    page.control_status(active_control.as_deref().unwrap_or("none"), message),
                ]
                .spacing(4),
            );
        }
        if vpns.is_empty() {
            body = body.push(super::kit::empty(fl!("settings-network-no-vpn")));
        }
        if forms.wireguard {
            body = body.push(
                container(
                    column![
                        text_field(forms, Field::WgName, fl!("settings-network-wg-name")),
                        secret(
                            "wg-private",
                            fl!("settings-network-private-key"),
                            forms.get(Field::WgPrivate),
                            |value| { edit(Edit::Text(Field::WgPrivate, value)) }
                        ),
                        text_field(
                            forms,
                            Field::WgAddress,
                            fl!("settings-network-wg-addresses")
                        ),
                        text_field(forms, Field::WgDns, fl!("settings-network-wg-dns")),
                        text_field(forms, Field::WgPeer, fl!("settings-network-wg-peer")),
                        text_field(
                            forms,
                            Field::WgEndpoint,
                            fl!("settings-network-wg-endpoint")
                        ),
                        text_field(forms, Field::WgAllowed, fl!("settings-network-wg-allowed")),
                        direction::row([
                            row_button(
                                "network-wg-add".into(),
                                fl!("settings-network-add"),
                                Some(edit(Edit::SubmitWireGuard)),
                            ),
                            row_button(
                                "network-wg-cancel".into(),
                                fl!("settings-cancel"),
                                Some(edit(Edit::WireGuard(false))),
                            ),
                        ])
                        .spacing(10),
                    ]
                    .spacing(10),
                )
                .padding(12)
                .width(Length::Fill)
                .style(super::kit::note),
            );
        } else {
            body = body.push(row_button(
                "network-wg-open".into(),
                fl!("settings-network-wg-open"),
                Some(edit(Edit::WireGuard(true))),
            ));
        }
        // Detection, WireGuard, then the plugins by their own names.
        let mut kinds = vec![
            (DETECT.to_owned(), fl!("settings-network-import-detect")),
            ("wireguard".to_owned(), "WireGuard".to_owned()),
        ];
        kinds.extend(
            state
                .overview
                .vpn_plugins
                .iter()
                .map(|plugin| (plugin.name.clone(), plugin.name.clone())),
        );
        body = body
            .push(text(fl!("settings-network-import-title")))
            .push(
                direction::row([
                    text_field(
                        forms,
                        Field::ImportPath,
                        fl!("settings-network-import-file"),
                    ),
                    row_button(
                        "network-import-choose".into(),
                        fl!("settings-network-choose"),
                        Some(edit(Edit::ChooseFile(Field::ImportPath))),
                    ),
                ])
                .spacing(10)
                .align_y(Alignment::End),
            )
            .push(
                direction::row([
                    pick(
                        "network-import-kind",
                        kinds,
                        Some(forms.import_kind.clone()),
                        |value| edit(Edit::Choose(Pick::ImportKind, value)),
                    ),
                    row_button(
                        "network-import".into(),
                        fl!("settings-network-import"),
                        Some(edit(Edit::SubmitImport)),
                    ),
                ])
                .spacing(10),
            )
            .push(muted(if state.overview.vpn_plugins.is_empty() {
                fl!("settings-network-import-no-plugins")
            } else {
                fl!("settings-network-import-plugins")
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
            muted(fl!("settings-network-hotspot-no-wifi"))
        } else if let Some(hotspot) = active {
            let path = hotspot
                .active
                .as_ref()
                .map(|(path, _)| path.clone())
                .unwrap_or_default();
            column![
                text(fl!(
                    "settings-network-hotspot-sharing",
                    name = hotspot.id.as_str()
                )),
                row_button(
                    "network-hotspot-stop".into(),
                    fl!("settings-network-hotspot-stop"),
                    Some(apply(NetworkAction::Deactivate(path.clone()))),
                ),
                self.network
                    .control_status(&format!("active-{path}"), message),
            ]
            .spacing(10)
            .into()
        } else {
            column![
                text_field(forms, Field::HotspotSsid, fl!("settings-network-name")),
                secret(
                    "hotspot-password",
                    fl!("settings-network-hotspot-password"),
                    forms.get(Field::HotspotPassword),
                    |value| { edit(Edit::Text(Field::HotspotPassword, value)) }
                ),
                row_button(
                    "network-hotspot".into(),
                    fl!("settings-network-hotspot-start"),
                    Some(edit(Edit::StartHotspot))
                ),
                muted(fl!("settings-network-hotspot-hint")),
                self.network.control_status("hotspot", message),
            ]
            .spacing(10)
            .into()
        };
        settings_group(fl!("settings-network-hotspot"), body)
    }

    fn connection_section<'a>(&'a self, state: &'a NetworkState) -> Element<'a, Message> {
        let page = &self.network;
        let forms = &self.network_forms;
        let connections: Vec<(String, String)> = state
            .overview
            .connections
            .iter()
            .map(|connection| (connection.path.clone(), connection_label(connection)))
            .collect();
        if connections.is_empty() {
            return settings_group(
                fl!("settings-network-connections"),
                super::kit::empty(fl!("settings-network-no-connections")),
            );
        }
        let selected = forms
            .selected
            .as_deref()
            .and_then(|path| state.overview.connection(path));
        let mut body = column![pick(
            "network-connection",
            connections,
            selected.map(|connection| connection.path.clone()),
            |path| edit(Edit::Select(path)),
        )]
        .spacing(10);
        let Some(connection) = selected else {
            return settings_group(
                fl!("settings-network-connections"),
                body.push(muted(fl!("settings-network-choose-connection"))),
            );
        };
        let target = Target::from(connection);
        let key = |kind: &str| format!("change-{}-{kind}", connection.uuid);
        let ip = |family: &'static str,
                  title: &'static str,
                  current: &str,
                  fields: [Field; 3]|
         -> Element<'a, Message> {
            let manual = current == "manual";
            let choose = if family == "ipv4" {
                Pick::Ip4
            } else {
                Pick::Ip6
            };
            let mut group = column![
                text(title),
                pick(
                    format!("network-{family}-method"),
                    options(&IP_METHODS, |id| ip_label(family, id)),
                    Some(current.to_owned()),
                    move |value| edit(Edit::Choose(choose, value)),
                ),
            ]
            .spacing(8);
            if manual {
                group = group
                    .push(text_field(
                        forms,
                        fields[0],
                        fl!("settings-network-ip-addresses"),
                    ))
                    .push(text_field(
                        forms,
                        fields[1],
                        fl!("settings-network-ip-gateway"),
                    ));
            }
            group
                .push(text_field(forms, fields[2], fl!("settings-network-ip-dns")))
                .push(row_button(
                    format!("network-{family}-apply"),
                    fl!("settings-network-apply"),
                    Some(edit(Edit::ApplyIp(family))),
                ))
                .push(page.control_status(&key(family), message))
                .into()
        };
        body = body
            .push(toggle(
                "network-autoconnect",
                fl!("settings-network-autoconnect"),
                connection.autoconnect,
                apply(NetworkAction::Change(
                    target.clone(),
                    Change::Autoconnect(!connection.autoconnect),
                )),
            ))
            .push(page.control_status(&key("autoconnect"), message))
            .push(
                column![
                    text(fl!("settings-network-metered")),
                    pick(
                        "network-metered",
                        METERED
                            .iter()
                            .map(|metered| (*metered, metered_label(*metered)))
                            .collect(),
                        Some(connection.metered),
                        {
                            let target = target.clone();
                            move |metered| {
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
                    &forms.ip4,
                    [Field::Ip4Address, Field::Ip4Gateway, Field::Ip4Dns],
                ))
                .push(ip(
                    "ipv6",
                    "IPv6",
                    &forms.ip6,
                    [Field::Ip6Address, Field::Ip6Gateway, Field::Ip6Dns],
                ));
        }
        let mut proxy = column![
            text(fl!("settings-network-proxy")),
            pick(
                "network-proxy",
                options(&PROXY, proxy_label),
                Some(forms.proxy.clone()),
                |value| { edit(Edit::Choose(Pick::Proxy, value)) }
            ),
        ]
        .spacing(8);
        match forms.proxy.as_str() {
            "manual" => {
                proxy = proxy.push(text_field(
                    forms,
                    Field::Proxy,
                    fl!("settings-network-proxy-server"),
                ))
            }
            "auto" => {
                proxy = proxy.push(text_field(
                    forms,
                    Field::Proxy,
                    fl!("settings-network-proxy-url"),
                ))
            }
            _ => {}
        }
        body = body
            .push(
                proxy
                    .push(row_button(
                        "network-proxy-apply".into(),
                        fl!("settings-network-apply"),
                        Some(edit(Edit::ApplyProxy)),
                    ))
                    .push(muted(fl!("settings-network-proxy-hint")))
                    .push(page.control_status(&key("proxy"), message)),
            )
            .push(row_button(
                "network-forget".into(),
                fl!("settings-network-forget"),
                Some(apply(NetworkAction::Forget(target))),
            ))
            .push(page.control_status(&format!("connection-{}", connection.uuid), message));
        settings_group(fl!("settings-network-connections"), body)
    }

    pub(super) fn network_view(&self) -> Element<'_, Message> {
        self.network.view(message, |state| {
            let mut body = column![].spacing(28);
            if let Some(question) = self.question_section(state) {
                body = body.push(question);
            }
            if !state.agent {
                body = body.push(muted(fl!("settings-network-no-agent")));
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
