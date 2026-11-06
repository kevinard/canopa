//! A connector label is a location; a live identity also distinguishes replacement.
use super::*;
use futures_util::{StreamExt, TryStreamExt, stream};
use zbus::zvariant::OwnedValue;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "Wire", into = "Wire")]
pub struct Monitor {
    name: String,
    identity: Option<u64>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Wire {
    Legacy(String),
    Pinned(Pinned),
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pinned {
    name: String,
    identity: u64,
}
impl Monitor {
    pub fn pinned(name: impl Into<String>, identity: u64) -> anyhow::Result<Self> {
        let monitor = Self {
            name: name.into(),
            identity: Some(identity),
        };
        anyhow::ensure!(monitor.valid(), "Invalid capture monitor identity");
        Ok(monitor)
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn identity(&self) -> Option<u64> {
        self.identity
    }
    pub(super) fn valid(&self) -> bool {
        !self.name.is_empty()
            && self.name.len() <= 256
            && !self.name.chars().any(char::is_control)
            && self.identity != Some(0)
    }
}
impl From<String> for Monitor {
    fn from(name: String) -> Self {
        Self {
            name,
            identity: None,
        }
    }
}
impl From<&str> for Monitor {
    fn from(name: &str) -> Self {
        name.to_owned().into()
    }
}
impl TryFrom<Wire> for Monitor {
    type Error = anyhow::Error;
    fn try_from(wire: Wire) -> Result<Self, Self::Error> {
        match wire {
            Wire::Legacy(name) => Ok(name.into()),
            Wire::Pinned(Pinned { name, identity }) => Self::pinned(name, identity),
        }
    }
}
impl From<Monitor> for Wire {
    fn from(monitor: Monitor) -> Self {
        match monitor.identity {
            Some(identity) => Self::Pinned(Pinned {
                name: monitor.name,
                identity,
            }),
            None => Self::Legacy(monitor.name),
        }
    }
}

pub(super) async fn supported(connection: &Connection, owner: &str) -> anyhow::Result<bool> {
    let result: anyhow::Result<OwnedValue> = worker::call(
        connection,
        owner,
        PATH,
        "org.freedesktop.DBus.Properties",
        "Get",
        &(SERVICE, "CanopaSourceVersion"),
        CALL_TIME,
    )
    .await;
    let value = match result {
        Ok(value) => value,
        Err(error) if error.downcast_ref::<zbus::Error>().is_some_and(|e| {
            matches!(e, zbus::Error::MethodError(name, _, _) if name.as_str() == "org.freedesktop.DBus.Error.UnknownProperty")
        }) => return Ok(false),
        Err(error) => return Err(error),
    };
    anyhow::ensure!(
        u32::try_from(value)? == 1,
        "Unsupported capture source identities"
    );
    Ok(true)
}

impl Owner {
    pub(crate) fn unique_name(&self) -> &str {
        self.0.as_str()
    }

    /// Preserve the order and identity of the inventory belonging to this owner.
    /// Only a missing extension permits legacy view-only capture by connector.
    pub async fn monitors(
        &self,
        connection: &Connection,
        names: Vec<String>,
    ) -> anyhow::Result<Vec<Monitor>> {
        anyhow::ensure!(names.len() <= MAX_SOURCES, "Too many capture monitors");
        anyhow::ensure!(
            names.iter().all(|name| Monitor::from(name.clone()).valid()),
            "Invalid capture monitor"
        );
        if names.is_empty() {
            return Ok(Vec::new());
        }
        if !supported(connection, self.0.as_str()).await? {
            return Ok(names.into_iter().map(Monitor::from).collect());
        }
        tokio::time::timeout(
            CALL_TIME,
            stream::iter(names.into_iter().map(|name| async move {
                let (identity, position, size): (u64, (i32, i32), (i32, i32)) = worker::call(
                    connection,
                    self.0.as_str(),
                    PATH,
                    SERVICE,
                    "MonitorInfo",
                    &(name.as_str(),),
                    CALL_TIME,
                )
                .await?;
                anyhow::ensure!(
                    Geometry { position, size }.valid(),
                    "Invalid live capture monitor geometry"
                );
                Monitor::pinned(name, identity)
            }))
            .buffered(4)
            .try_collect(),
        )
        .await?
    }
}
