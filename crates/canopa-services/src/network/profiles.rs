//! New NetworkManager connection profiles: Wi-Fi (open, personal and
//! WPA-Enterprise with certificates), hotspots and WireGuard, including the
//! WireGuard `.conf` format that `wg-quick` and VPN providers use.
//!
//! Secrets the person typed are handed to NetworkManager in the profile with
//! flags that say who keeps them: [`SecretStorage::System`] lets
//! NetworkManager store them; [`SecretStorage::Ask`] stores nothing and
//! NetworkManager asks a secret agent each time it connects
//! (see [`super::secret_agent`]).
use super::manager::Settings;
use crate::Error;
use std::collections::HashMap;
use zbus::zvariant::{OwnedValue, Value};

fn owned(value: impl Into<Value<'static>>) -> OwnedValue {
    value
        .into()
        .try_into()
        .expect("plain values have no file descriptors")
}

fn uuid() -> String {
    // A random version 4 UUID from the kernel's generator.
    std::fs::read_to_string("/proc/sys/kernel/random/uuid")
        .map(|value| value.trim().to_owned())
        .unwrap_or_else(|_| {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default();
            format!(
                "{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
                (nanos >> 64) as u32,
                (nanos >> 48) as u16,
                (nanos >> 36) as u16 & 0xfff,
                (nanos >> 24) as u16 & 0xfff,
                nanos as u64 & 0xffff_ffff_ffff
            )
        })
}

/// Who keeps a secret the person typed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SecretStorage {
    /// NetworkManager stores it with the profile (flags 0).
    #[default]
    System,
    /// Nobody stores it; NetworkManager asks a secret agent when connecting
    /// (flags 2, not saved).
    Ask,
}

impl SecretStorage {
    fn flags(self) -> u32 {
        match self {
            Self::System => 0,
            Self::Ask => 2,
        }
    }
}

/// The inner method of PEAP and TTLS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase2 {
    Mschapv2,
    Pap,
    Gtc,
    Mschap,
    Chap,
}

impl Phase2 {
    pub const ALL: [Self; 5] = [
        Self::Mschapv2,
        Self::Pap,
        Self::Gtc,
        Self::Mschap,
        Self::Chap,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Mschapv2 => "mschapv2",
            Self::Pap => "pap",
            Self::Gtc => "gtc",
            Self::Mschap => "mschap",
            Self::Chap => "chap",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EapMethod {
    Peap,
    Ttls,
    Tls,
}

impl EapMethod {
    pub const ALL: [Self; 3] = [Self::Peap, Self::Ttls, Self::Tls];
    pub fn id(self) -> &'static str {
        match self {
            Self::Peap => "peap",
            Self::Ttls => "ttls",
            Self::Tls => "tls",
        }
    }
}

/// WPA-Enterprise (802.1X) sign-in.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Enterprise {
    pub method: Option<EapMethod>,
    pub phase2: Option<Phase2>,
    pub identity: String,
    pub anonymous_identity: String,
    /// The server's CA certificate file; `None` uses the system's CAs.
    pub ca_certificate: Option<std::path::PathBuf>,
    /// The server name the certificate must match (`domain-suffix-match`).
    pub domain: String,
    /// PEAP and TTLS.
    pub password: String,
    /// TLS: the client certificate and its private key.
    pub client_certificate: Option<std::path::PathBuf>,
    pub private_key: Option<std::path::PathBuf>,
    pub private_key_password: String,
    pub storage: SecretStorage,
}

impl std::fmt::Debug for Enterprise {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Enterprise")
            .field("method", &self.method)
            .field("phase2", &self.phase2)
            .field("identity", &self.identity)
            .field("ca_certificate", &self.ca_certificate)
            .field("domain", &self.domain)
            .field("client_certificate", &self.client_certificate)
            .field("private_key", &self.private_key)
            .field("storage", &self.storage)
            .finish_non_exhaustive()
    }
}

/// A Wi-Fi network's security for a new profile.
#[derive(Clone, PartialEq, Eq)]
pub enum WifiSecurity {
    Open,
    Personal {
        password: String,
        /// WPA3-Personal.
        sae: bool,
        storage: SecretStorage,
    },
    Enterprise(Enterprise),
}

impl std::fmt::Debug for WifiSecurity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => formatter.write_str("Open"),
            Self::Personal { sae, storage, .. } => formatter
                .debug_struct("Personal")
                .field("sae", sae)
                .field("storage", storage)
                .finish_non_exhaustive(),
            Self::Enterprise(enterprise) => enterprise.fmt(formatter),
        }
    }
}

fn certificate(path: &std::path::Path) -> Result<OwnedValue, Error> {
    if !path.is_absolute() || !path.is_file() {
        return Err(Error::Invalid(
            "Choose a certificate or key file that exists.",
        ));
    }
    let path = path
        .to_str()
        .ok_or(Error::Invalid("Certificate paths must be UTF-8."))?;
    // NetworkManager's path scheme: "file://" + path + NUL.
    let mut bytes = format!("file://{path}").into_bytes();
    bytes.push(0);
    Ok(owned(bytes))
}

fn connection(id: &str, kind: &str, autoconnect: bool) -> HashMap<String, OwnedValue> {
    HashMap::from([
        ("id".into(), owned(id.to_owned())),
        ("uuid".into(), owned(uuid())),
        ("type".into(), owned(kind.to_owned())),
        ("autoconnect".into(), owned(autoconnect)),
    ])
}

fn automatic_ip() -> [(String, HashMap<String, OwnedValue>); 2] {
    [
        (
            "ipv4".into(),
            HashMap::from([("method".into(), owned("auto"))]),
        ),
        (
            "ipv6".into(),
            HashMap::from([("method".into(), owned("auto"))]),
        ),
    ]
}

/// A Wi-Fi profile for `ssid`. `hidden` networks are probed for by name.
pub fn wifi(ssid: &[u8], security: &WifiSecurity, hidden: bool) -> Result<Settings, Error> {
    if ssid.is_empty() || ssid.len() > 32 {
        return Err(Error::Invalid("A network name has 1 to 32 bytes."));
    }
    let name = String::from_utf8_lossy(ssid).into_owned();
    let mut settings: Settings = HashMap::from([
        (
            "connection".into(),
            connection(&name, "802-11-wireless", true),
        ),
        (
            "802-11-wireless".into(),
            HashMap::from([
                ("ssid".into(), owned(ssid.to_vec())),
                ("mode".into(), owned("infrastructure")),
                ("hidden".into(), owned(hidden)),
            ]),
        ),
    ]);
    settings.extend(automatic_ip());
    match security {
        WifiSecurity::Open => {}
        WifiSecurity::Personal {
            password,
            sae,
            storage,
        } => {
            let valid = (8..=63).contains(&password.len())
                || (password.len() == 64 && password.bytes().all(|b| b.is_ascii_hexdigit()));
            if !valid {
                return Err(Error::Invalid("A Wi-Fi password has 8 to 63 characters."));
            }
            settings.insert(
                "802-11-wireless-security".into(),
                HashMap::from([
                    (
                        "key-mgmt".into(),
                        owned(if *sae { "sae" } else { "wpa-psk" }),
                    ),
                    ("psk".into(), owned(password.clone())),
                    ("psk-flags".into(), owned(storage.flags())),
                ]),
            );
        }
        WifiSecurity::Enterprise(enterprise) => {
            settings.insert(
                "802-11-wireless-security".into(),
                HashMap::from([("key-mgmt".into(), owned("wpa-eap"))]),
            );
            settings.insert("802-1x".into(), eap(enterprise)?);
        }
    }
    Ok(settings)
}

/// The `802-1x` setting for WPA-Enterprise.
pub fn eap(enterprise: &Enterprise) -> Result<HashMap<String, OwnedValue>, Error> {
    let method = enterprise
        .method
        .ok_or(Error::Invalid("Choose an authentication method."))?;
    if enterprise.identity.trim().is_empty() || enterprise.identity.len() > 256 {
        return Err(Error::Invalid("Enter your identity (user name)."));
    }
    let mut values = HashMap::from([
        ("eap".into(), owned(vec![method.id().to_owned()])),
        (
            "identity".into(),
            owned(enterprise.identity.trim().to_owned()),
        ),
    ]);
    if !enterprise.anonymous_identity.trim().is_empty() {
        values.insert(
            "anonymous-identity".into(),
            owned(enterprise.anonymous_identity.trim().to_owned()),
        );
    }
    match &enterprise.ca_certificate {
        Some(path) => {
            values.insert("ca-cert".into(), certificate(path)?);
        }
        None => {
            values.insert("system-ca-certs".into(), owned(true));
        }
    }
    if !enterprise.domain.trim().is_empty() {
        values.insert(
            "domain-suffix-match".into(),
            owned(enterprise.domain.trim().to_owned()),
        );
    }
    match method {
        EapMethod::Peap | EapMethod::Ttls => {
            let phase2 = enterprise.phase2.unwrap_or(Phase2::Mschapv2);
            values.insert("phase2-auth".into(), owned(phase2.id()));
            values.insert("password-flags".into(), owned(enterprise.storage.flags()));
            if enterprise.storage == SecretStorage::System {
                if enterprise.password.is_empty() {
                    return Err(Error::Invalid("Enter the password."));
                }
                values.insert("password".into(), owned(enterprise.password.clone()));
            }
        }
        EapMethod::Tls => {
            let (Some(certificate_path), Some(key)) =
                (&enterprise.client_certificate, &enterprise.private_key)
            else {
                return Err(Error::Invalid(
                    "TLS needs your certificate and private key.",
                ));
            };
            values.insert("client-cert".into(), certificate(certificate_path)?);
            values.insert("private-key".into(), certificate(key)?);
            values.insert(
                "private-key-password-flags".into(),
                owned(enterprise.storage.flags()),
            );
            if enterprise.storage == SecretStorage::System
                && !enterprise.private_key_password.is_empty()
            {
                values.insert(
                    "private-key-password".into(),
                    owned(enterprise.private_key_password.clone()),
                );
            }
        }
    }
    Ok(values)
}

/// A Wi-Fi hotspot that shares this computer's connection.
pub fn hotspot(ssid: &str, password: &str) -> Result<Settings, Error> {
    if ssid.is_empty() || ssid.len() > 32 {
        return Err(Error::Invalid("A network name has 1 to 32 bytes."));
    }
    if !(8..=63).contains(&password.len()) {
        return Err(Error::Invalid("A hotspot password has 8 to 63 characters."));
    }
    Ok(HashMap::from([
        (
            "connection".into(),
            connection("Hotspot", "802-11-wireless", false),
        ),
        (
            "802-11-wireless".into(),
            HashMap::from([
                ("ssid".into(), owned(ssid.as_bytes().to_vec())),
                ("mode".into(), owned("ap")),
                ("band".into(), owned("bg")),
            ]),
        ),
        (
            "802-11-wireless-security".into(),
            HashMap::from([
                ("key-mgmt".into(), owned("wpa-psk")),
                ("psk".into(), owned(password.to_owned())),
                ("proto".into(), owned(vec!["rsn".to_owned()])),
                ("pairwise".into(), owned(vec!["ccmp".to_owned()])),
                ("group".into(), owned(vec!["ccmp".to_owned()])),
            ]),
        ),
        (
            "ipv4".into(),
            HashMap::from([("method".into(), owned("shared"))]),
        ),
        (
            "ipv6".into(),
            HashMap::from([("method".into(), owned("ignore"))]),
        ),
    ]))
}

/// One WireGuard peer.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Peer {
    pub public_key: String,
    pub preshared_key: String,
    pub endpoint: String,
    pub allowed_ips: Vec<String>,
    pub keepalive: u32,
}

/// A WireGuard tunnel.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct WireGuard {
    pub name: String,
    pub private_key: String,
    pub addresses: Vec<String>,
    pub dns: Vec<String>,
    pub listen_port: u32,
    pub mtu: u32,
    pub peers: Vec<Peer>,
}

impl std::fmt::Debug for WireGuard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WireGuard")
            .field("name", &self.name)
            .field("addresses", &self.addresses)
            .field("dns", &self.dns)
            .field("peers", &self.peers.len())
            .finish_non_exhaustive()
    }
}

fn key(value: &str) -> bool {
    // A base64 Curve25519 key: 43 characters and one '='.
    value.len() == 44
        && value.ends_with('=')
        && value[..43]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
}

/// Read a WireGuard `.conf` (the `wg-quick` format). `name` names the
/// connection and, shortened to 15 characters, its interface.
pub fn parse_wireguard(text: &str, name: &str) -> Result<WireGuard, Error> {
    let mut tunnel = WireGuard {
        name: name.trim().chars().take(64).collect(),
        ..Default::default()
    };
    let mut section = "";
    for line in text.lines().take(1000) {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            section = match line.to_ascii_lowercase().as_str() {
                "[interface]" => "interface",
                "[peer]" => {
                    tunnel.peers.push(Peer::default());
                    "peer"
                }
                _ => return Err(Error::Invalid("The file has an unknown section.")),
            };
            continue;
        }
        let Some((field, value)) = line.split_once('=') else {
            return Err(Error::Invalid("The file is not a WireGuard configuration."));
        };
        let (field, value) = (field.trim().to_ascii_lowercase(), value.trim().to_owned());
        let list = || -> Vec<String> {
            value
                .split(',')
                .map(|part| part.trim().to_owned())
                .filter(|part| !part.is_empty())
                .collect()
        };
        match (section, field.as_str()) {
            ("interface", "privatekey") => tunnel.private_key = value.clone(),
            ("interface", "address") => tunnel.addresses.extend(list()),
            ("interface", "dns") => tunnel.dns.extend(list()),
            ("interface", "listenport") => {
                tunnel.listen_port = value
                    .parse()
                    .map_err(|_| Error::Invalid("ListenPort is not a port."))?
            }
            ("interface", "mtu") => {
                tunnel.mtu = value
                    .parse()
                    .map_err(|_| Error::Invalid("MTU is not a number."))?
            }
            // wg-quick hooks and routing tables have no NetworkManager equivalent.
            ("interface", "table" | "preup" | "postup" | "predown" | "postdown" | "saveconfig") => {
            }
            ("peer", field) => {
                let peer = tunnel.peers.last_mut().expect("a peer section is open");
                match field {
                    "publickey" => peer.public_key = value.clone(),
                    "presharedkey" => peer.preshared_key = value.clone(),
                    "endpoint" => peer.endpoint = value.clone(),
                    "allowedips" => peer.allowed_ips.extend(list()),
                    "persistentkeepalive" => {
                        peer.keepalive = value
                            .parse()
                            .map_err(|_| Error::Invalid("PersistentKeepalive is not a number."))?
                    }
                    _ => return Err(Error::Invalid("The file has an unknown peer setting.")),
                }
            }
            _ => return Err(Error::Invalid("The file is not a WireGuard configuration.")),
        }
    }
    Ok(tunnel)
}

/// The interface name for a tunnel: `wg-` and its name's letters, at most
/// 15 characters.
fn interface(name: &str) -> String {
    let base: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(12)
        .collect();
    if base.is_empty() {
        "wg0".into()
    } else if base.starts_with("wg") {
        base
    } else {
        format!("wg-{base}").chars().take(15).collect()
    }
}

/// A WireGuard profile. The private key is stored by NetworkManager.
pub fn wireguard(tunnel: &WireGuard) -> Result<Settings, Error> {
    if tunnel.name.trim().is_empty() {
        return Err(Error::Invalid("Name the connection."));
    }
    if !key(&tunnel.private_key) {
        return Err(Error::Invalid("The private key is not a WireGuard key."));
    }
    if tunnel.peers.is_empty() {
        return Err(Error::Invalid("A tunnel needs a peer."));
    }
    let mut peers = Vec::new();
    for peer in &tunnel.peers {
        if !key(&peer.public_key) {
            return Err(Error::Invalid(
                "A peer's public key is not a WireGuard key.",
            ));
        }
        if !peer.preshared_key.is_empty() && !key(&peer.preshared_key) {
            return Err(Error::Invalid("A preshared key is not a WireGuard key."));
        }
        let mut values: HashMap<String, Value<'static>> = HashMap::from([
            (
                "public-key".to_string(),
                Value::from(peer.public_key.clone()),
            ),
            (
                "allowed-ips".to_string(),
                Value::from(peer.allowed_ips.clone()),
            ),
        ]);
        if !peer.endpoint.is_empty() {
            values.insert("endpoint".into(), Value::from(peer.endpoint.clone()));
        }
        if !peer.preshared_key.is_empty() {
            values.insert(
                "preshared-key".into(),
                Value::from(peer.preshared_key.clone()),
            );
            values.insert("preshared-key-flags".into(), Value::from(0u32));
        }
        if peer.keepalive > 0 {
            values.insert("persistent-keepalive".into(), Value::from(peer.keepalive));
        }
        peers.push(values);
    }
    let mut wireguard = HashMap::from([
        ("private-key".into(), owned(tunnel.private_key.clone())),
        ("private-key-flags".into(), owned(0u32)),
        (
            "peers".into(),
            OwnedValue::try_from(Value::from(peers)).expect("plain values"),
        ),
    ]);
    if tunnel.listen_port > 0 {
        wireguard.insert("listen-port".into(), owned(tunnel.listen_port));
    }
    if tunnel.mtu > 0 {
        wireguard.insert("mtu".into(), owned(tunnel.mtu));
    }
    let mut main = connection(tunnel.name.trim(), "wireguard", false);
    main.insert("interface-name".into(), owned(interface(&tunnel.name)));
    let mut settings: Settings =
        HashMap::from([("connection".into(), main), ("wireguard".into(), wireguard)]);
    for (family, v6) in [("ipv4", false), ("ipv6", true)] {
        let addresses: Vec<String> = tunnel
            .addresses
            .iter()
            .filter(|address| address.contains(':') == v6)
            .cloned()
            .collect();
        let dns: Vec<String> = tunnel
            .dns
            .iter()
            .filter(|server| {
                server.contains(':') == v6 && server.parse::<std::net::IpAddr>().is_ok()
            })
            .cloned()
            .collect();
        let mut ip = Settings::new();
        let method = if addresses.is_empty() {
            if v6 { "ignore" } else { "disabled" }
        } else {
            "manual"
        };
        super::manager::apply_change(
            &mut ip,
            &super::manager::Change::Ip(
                family,
                super::manager::IpConfig {
                    method: method.into(),
                    addresses,
                    gateway: None,
                    dns,
                },
            ),
        )?;
        settings.extend(ip);
    }
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONF: &str = "[Interface]\nPrivateKey = yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=\nAddress = 10.0.0.2/24, fd00::2/64\nDNS = 10.0.0.1\nPostUp = iptables -A FORWARD\n\n[Peer]\nPublicKey = xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=\nAllowedIPs = 0.0.0.0/0, ::/0\nEndpoint = vpn.example.com:51820\nPersistentKeepalive = 25 # comment\n";

    #[test]
    fn wireguard_files_become_profiles() {
        let tunnel = parse_wireguard(CONF, "Office").unwrap();
        assert_eq!(tunnel.addresses, ["10.0.0.2/24", "fd00::2/64"]);
        assert_eq!(tunnel.peers.len(), 1);
        assert_eq!(tunnel.peers[0].keepalive, 25);
        let settings = wireguard(&tunnel).unwrap();
        assert_eq!(
            <&str>::try_from(&settings["connection"]["interface-name"]).unwrap(),
            "wg-Office"
        );
        assert_eq!(
            <&str>::try_from(&settings["ipv4"]["method"]).unwrap(),
            "manual"
        );
        assert_eq!(
            <&str>::try_from(&settings["ipv6"]["method"]).unwrap(),
            "manual"
        );
        assert!(settings["wireguard"].contains_key("peers"));
        assert!(parse_wireguard("[Interface]\nBogus = 1\n", "x").is_err());
        assert!(parse_wireguard("not a conf", "x").is_err());
        let mut broken = tunnel.clone();
        broken.private_key = "short".into();
        assert!(wireguard(&broken).is_err());
        assert!(
            format!("{tunnel:?}").contains("Office") && !format!("{tunnel:?}").contains("yAnz")
        );
    }

    #[test]
    fn wifi_profiles_carry_secrets_with_their_storage_flags() {
        let personal = wifi(
            b"Home",
            &WifiSecurity::Personal {
                password: "correct horse".into(),
                sae: false,
                storage: SecretStorage::System,
            },
            true,
        )
        .unwrap();
        assert_eq!(
            <&str>::try_from(&personal["802-11-wireless-security"]["psk"]).unwrap(),
            "correct horse"
        );
        assert!(bool::try_from(&personal["802-11-wireless"]["hidden"]).unwrap());
        assert!(
            wifi(
                b"Home",
                &WifiSecurity::Personal {
                    password: "short".into(),
                    sae: false,
                    storage: SecretStorage::System
                },
                false
            )
            .is_err()
        );
        let ca = tempfile::NamedTempFile::new().unwrap();
        let enterprise = Enterprise {
            method: Some(EapMethod::Peap),
            identity: "me@example.edu".into(),
            ca_certificate: Some(ca.path().into()),
            domain: "radius.example.edu".into(),
            password: "secret".into(),
            storage: SecretStorage::Ask,
            ..Default::default()
        };
        let settings = wifi(
            b"eduroam",
            &WifiSecurity::Enterprise(enterprise.clone()),
            false,
        )
        .unwrap();
        let eap = &settings["802-1x"];
        assert_eq!(<&str>::try_from(&eap["phase2-auth"]).unwrap(), "mschapv2");
        assert_eq!(u32::try_from(&eap["password-flags"]).unwrap(), 2);
        // Asked each time: the password is not in the profile.
        assert!(!eap.contains_key("password"));
        let bytes = Vec::<u8>::try_from(eap["ca-cert"].try_clone().unwrap()).unwrap();
        assert!(bytes.starts_with(b"file:///") && bytes.ends_with(&[0]));
        assert!(!format!("{enterprise:?}").contains("secret"));
        let tls = Enterprise {
            method: Some(EapMethod::Tls),
            identity: "me".into(),
            ..Default::default()
        };
        assert!(eap_err(&tls).contains("certificate"));
        assert!(hotspot("Share", "short").is_err());
        let share = hotspot("Share", "long enough").unwrap();
        assert_eq!(
            <&str>::try_from(&share["ipv4"]["method"]).unwrap(),
            "shared"
        );
    }

    fn eap_err(enterprise: &Enterprise) -> String {
        eap(enterprise).unwrap_err().to_string()
    }
}
