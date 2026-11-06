//! Request-owned credential transport. Secrets never use the worker's JSON
//! input/output, application settings, command line or recovery journal.
mod broker;
mod channel;
mod handoff;
mod prompt;
mod secret;
#[cfg(test)]
mod tests;
pub(crate) use broker::{Broker, Retained};
pub(crate) use channel::{CacheOnly, Channel, Scope, current};
pub(crate) use secret::{Credentials, Secret};

use super::worker::Destination;
use serde::{Deserialize, Serialize};

pub(crate) const MAX_FRAME: usize = 8192;
// Cached replies consume exchanges, not interactive prompt attempts. A bounded
// recovery batch can use the same server credentials for several records.
pub(crate) const MAX_EXCHANGES: u32 = 64;
pub(crate) const MAX_ATTEMPTS: u8 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    Domain,
    Username,
    Password,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Requirement {
    #[default]
    None,
    Negotiate,
    Fields {
        fields: Vec<Field>,
    },
}
impl Requirement {
    pub(crate) fn parse(value: Option<&str>) -> anyhow::Result<Self> {
        let Some(value) = value else {
            return Ok(Self::None);
        };
        let result = match value {
            "none" => Self::None,
            "negotiate" => Self::Negotiate,
            _ => Self::Fields {
                fields: value
                    .split(',')
                    .map(|s| match s {
                        "domain" => Ok(Field::Domain),
                        "username" => Ok(Field::Username),
                        "password" => Ok(Field::Password),
                        _ => anyhow::bail!("Unsupported printer authentication requirement"),
                    })
                    .collect::<anyhow::Result<_>>()?,
            },
        };
        result.validate()?;
        Ok(result)
    }
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        if let Self::Fields { fields } = self {
            anyhow::ensure!(
                !fields.is_empty()
                    && fields.len() <= 3
                    && fields
                        .iter()
                        .enumerate()
                        .all(|(n, f)| !fields[..n].contains(f)),
                "Invalid printer authentication fields"
            );
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Transport {
    Local,
    Loopback,
    Tls,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Endpoint {
    pub server: String,
    pub port: u16,
    pub transport: Transport,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ServerLimits {
    #[default]
    Legacy,
    CanopaV1,
}
impl ServerLimits {
    fn is_legacy(&self) -> bool {
        *self == Self::Legacy
    }
    pub(crate) fn username_bytes(self) -> usize {
        match self {
            Self::Legacy => 64,
            Self::CanopaV1 => 127,
        }
    }
    pub(crate) fn combined_bytes(self) -> usize {
        match self {
            Self::Legacy => 189,
            Self::CanopaV1 => 1151,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Kind {
    Server {
        realm: String,
        resource: String,
        #[serde(default, skip_serializing_if = "ServerLimits::is_legacy")]
        limits: ServerLimits,
    },
    Queue {
        destination: Destination,
        fields: Vec<Field>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Challenge {
    pub version: u8,
    #[serde(default = "default_interactive")]
    pub interactive: bool,
    pub id: u32,
    pub endpoint: Endpoint,
    pub kind: Kind,
    pub attempt: u8,
    pub suggested_user: String,
}
fn default_interactive() -> bool {
    true
}
fn text(value: &str, maximum: usize) -> bool {
    value.len() <= maximum && !value.chars().any(char::is_control)
}
impl Challenge {
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.version == 1
                && (1..=MAX_EXCHANGES).contains(&self.id)
                && (1..=MAX_ATTEMPTS).contains(&self.attempt)
                && !self.endpoint.server.is_empty()
                && text(&self.endpoint.server, 4096)
                && self.endpoint.port > 0
                && text(&self.suggested_user, 127),
            "Invalid printer authentication challenge"
        );
        match &self.kind {
            Kind::Server {
                realm, resource, ..
            } => anyhow::ensure!(
                text(realm, 256) && resource.starts_with('/') && text(resource, 1024),
                "Invalid server authentication scope"
            ),
            Kind::Queue {
                destination,
                fields,
            } => {
                destination.validate()?;
                Requirement::Fields {
                    fields: fields.clone(),
                }
                .validate()?;
            }
        }
        Ok(())
    }
    pub(crate) fn fields(&self) -> &[Field] {
        match &self.kind {
            Kind::Server { .. } => &[Field::Username, Field::Password],
            Kind::Queue { fields, .. } => fields,
        }
    }
}
