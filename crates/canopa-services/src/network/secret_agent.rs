//! A NetworkManager secret agent that asks the person and keeps nothing.
//!
//! NetworkManager asks registered agents for secrets it does not store: a
//! password marked "ask every time", a rejected password it wants again, or a
//! VPN's one-time code. [`register`] serves
//! `org.freedesktop.NetworkManager.SecretAgent` on a system bus connection
//! and registers it; each `GetSecrets` call becomes a [`SecretRequest`] on
//! the returned channel, which the page answers or drops. Requests that do
//! not allow interaction are refused at once, because this agent has no store.
//! `SaveSecrets` and `DeleteSecrets` succeed without keeping anything:
//! NetworkManager keeps system-owned secrets itself. Only calls from
//! NetworkManager's current unique name are answered. See
//! `docs/architecture/settings.md`.
//!
//! ```no_run
//! use canopa_services::network::secret_agent;
//! # async fn page() -> Result<(), canopa_services::Error> {
//! let system = canopa_services::bus::system().await?;
//! let (_agent, mut requests) = secret_agent::register(&system, "org.canopa.Settings").await?;
//! while let Some(request) = requests.recv().await {
//!     let answers = request
//!         .fields
//!         .iter()
//!         .map(|field| (field.key.clone(), "typed".to_owned()))
//!         .collect();
//!     request.answer(answers);
//! }
//! # Ok(()) }
//! ```
use super::manager::{SERVICE, Settings};
use crate::Error;
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

pub const AGENT_PATH: &str = "/org/freedesktop/NetworkManager/SecretAgent";
const AGENT_MANAGER: &str = "/org/freedesktop/NetworkManager/AgentManager";
/// How long a question waits for the person.
const ANSWER: std::time::Duration = std::time::Duration::from_secs(120);
/// `GetSecrets` flags.
const ALLOW_INTERACTION: u32 = 0x1;
const REQUEST_NEW: u32 = 0x2;

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.NetworkManager.SecretAgent")]
pub enum AgentError {
    /// The person cancelled or did not answer.
    UserCanceled,
    /// This agent has no answer without asking.
    NoSecrets,
    /// The caller is not NetworkManager.
    PermissionDenied,
}

/// One secret NetworkManager needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretField {
    /// The property in the setting, such as `psk` or `password`.
    pub key: String,
    pub label: String,
}

/// A question from NetworkManager for the page to show.
#[derive(Debug)]
pub struct SecretRequest {
    pub connection: String,
    pub uuid: String,
    /// The setting that needs secrets, such as `802-1x` or `vpn`.
    pub setting: String,
    pub fields: Vec<SecretField>,
    /// NetworkManager rejected the previous answer.
    pub retry: bool,
    reply: oneshot::Sender<Option<HashMap<String, String>>>,
}

impl SecretRequest {
    /// Answer with the values typed for [`SecretRequest::fields`]; they are
    /// handed to NetworkManager and dropped here.
    pub fn answer(self, values: HashMap<String, String>) {
        let _ = self.reply.send(Some(values));
    }

    pub fn cancel(self) {
        let _ = self.reply.send(None);
    }
}

fn text(settings: &Settings, setting: &str, key: &str) -> Option<String> {
    settings
        .get(setting)?
        .get(key)
        .and_then(|value| <&str>::try_from(value).ok())
        .map(|value| crate::bus::bounded_text(value, 128))
}

/// The secrets a setting needs, from the connection's own settings.
pub fn fields(settings: &Settings, setting: &str, hints: &[String]) -> Vec<SecretField> {
    let field = |key: &str, label: String| SecretField {
        key: key.into(),
        label,
    };
    match setting {
        "802-11-wireless-security" => match text(settings, setting, "key-mgmt").as_deref() {
            Some("none") => vec![field("wep-key0", fl!("services-secret-wep-key"))],
            _ => vec![field("psk", fl!("services-secret-wifi-password"))],
        },
        "802-1x" => {
            let eap = settings
                .get("802-1x")
                .and_then(|values| values.get("eap"))
                .and_then(|value| Vec::<String>::try_from(value.try_clone().ok()?).ok())
                .unwrap_or_default();
            if eap.iter().any(|method| method == "tls") {
                vec![field(
                    "private-key-password",
                    fl!("services-secret-private-key-password"),
                )]
            } else {
                vec![field("password", fl!("services-secret-password"))]
            }
        }
        "wireguard" => vec![field("private-key", fl!("services-secret-wireguard-key"))],
        "vpn" => {
            // Plugins name the secrets they want in the hints.
            let mut asked: Vec<SecretField> = hints
                .iter()
                .filter(|hint| !hint.starts_with("x-"))
                .take(4)
                .map(|hint| field(hint, hint.replace('-', " ")))
                .collect();
            if asked.is_empty() {
                asked.push(field("password", fl!("services-secret-vpn-password")));
            }
            asked
        }
        _ => vec![field("password", fl!("services-secret-password"))],
    }
}

fn secrets(setting: &str, values: HashMap<String, String>) -> Settings {
    let owned = |value: Value<'static>| OwnedValue::try_from(value).expect("plain values");
    let values = if setting == "vpn" {
        // VPN plugin secrets are one string map under "secrets".
        HashMap::from([("secrets".to_string(), owned(Value::from(values)))])
    } else {
        values
            .into_iter()
            .map(|(key, value)| (key, owned(Value::from(value))))
            .collect()
    };
    HashMap::from([(setting.to_owned(), values)])
}

struct Agent {
    requests: mpsc::Sender<SecretRequest>,
    conn: zbus::Connection,
}

impl Agent {
    async fn sent_by_network_manager(&self, header: &zbus::message::Header<'_>) -> bool {
        let Some(sender) = header.sender() else {
            return false;
        };
        matches!(
            crate::bus::owner(&self.conn, SERVICE).await,
            Ok(Some(owner)) if owner.as_str() == sender.as_str()
        )
    }
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.SecretAgent")]
impl Agent {
    async fn get_secrets(
        &self,
        connection: Settings,
        _path: OwnedObjectPath,
        setting_name: String,
        hints: Vec<String>,
        flags: u32,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<Settings, AgentError> {
        if !self.sent_by_network_manager(&header).await {
            return Err(AgentError::PermissionDenied);
        }
        if flags & ALLOW_INTERACTION == 0 {
            return Err(AgentError::NoSecrets);
        }
        let (reply, answer) = oneshot::channel();
        let request = SecretRequest {
            connection: text(&connection, "connection", "id").unwrap_or_default(),
            uuid: text(&connection, "connection", "uuid").unwrap_or_default(),
            fields: fields(&connection, &setting_name, &hints),
            setting: setting_name.clone(),
            retry: flags & REQUEST_NEW != 0,
            reply,
        };
        // A page that is not listening, or one question already waiting, is a cancel.
        self.requests
            .try_send(request)
            .map_err(|_| AgentError::UserCanceled)?;
        match tokio::time::timeout(ANSWER, answer).await {
            Ok(Ok(Some(values))) => Ok(secrets(&setting_name, values)),
            _ => Err(AgentError::UserCanceled),
        }
    }

    async fn cancel_get_secrets(&self, _path: OwnedObjectPath, _setting_name: String) {
        // The page drops its question when the request future goes away.
    }

    async fn save_secrets(&self, _connection: Settings, _path: OwnedObjectPath) {}

    async fn delete_secrets(&self, _connection: Settings, _path: OwnedObjectPath) {}
}

/// An agent registration; dropping it unregisters the agent.
pub struct Registration {
    conn: zbus::Connection,
}

impl Drop for Registration {
    fn drop(&mut self) {
        let conn = self.conn.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                if let Ok(manager) = zbus::Proxy::new(
                    &conn,
                    SERVICE,
                    AGENT_MANAGER,
                    "org.freedesktop.NetworkManager.AgentManager",
                )
                .await
                {
                    let _ = manager.call::<_, _, ()>("Unregister", &()).await;
                }
                let _ = conn.object_server().remove::<Agent, _>(AGENT_PATH).await;
            });
        }
    }
}

/// Serve the agent on `conn` and register it with NetworkManager under
/// `identifier`. Questions arrive on the receiver, one at a time.
pub async fn register(
    conn: &zbus::Connection,
    identifier: &str,
) -> Result<(Registration, mpsc::Receiver<SecretRequest>), Error> {
    let (requests, receiver) = mpsc::channel(1);
    conn.object_server()
        .at(
            AGENT_PATH,
            Agent {
                requests,
                conn: conn.clone(),
            },
        )
        .await?;
    let manager = zbus::Proxy::new(
        conn,
        SERVICE,
        AGENT_MANAGER,
        "org.freedesktop.NetworkManager.AgentManager",
    )
    .await?;
    let registered = tokio::time::timeout(
        crate::bus::DEADLINE,
        manager.call::<_, _, ()>("Register", &(identifier,)),
    )
    .await;
    if !matches!(registered, Ok(Ok(()))) {
        let _ = conn.object_server().remove::<Agent, _>(AGENT_PATH).await;
        return Err(match registered {
            Ok(Err(error)) => error.into(),
            _ => Error::TimedOut,
        });
    }
    Ok((Registration { conn: conn.clone() }, receiver))
}
