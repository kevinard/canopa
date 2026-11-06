//! Owned compositor recordings. Consent/session UI belongs to the portal layer.
use crate::process::DialogProcess;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, sync::Arc, time::Duration};
use tokio::sync::{Semaphore, oneshot, watch};
use zbus::{Connection, names::OwnedUniqueName};

pub(crate) mod bindings;
mod monitor;
pub use bindings::{Bindings, Mapping};
#[cfg(test)]
pub(crate) mod tests;
mod transport;
mod worker;
pub use monitor::Monitor;

pub(crate) const SERVICE: &str = "org.gnome.Mutter.ScreenCast";
const PATH: &str = "/org/gnome/Mutter/ScreenCast";
const SESSION: &str = "org.gnome.Mutter.ScreenCast.Session";
const STREAM: &str = "org.gnome.Mutter.ScreenCast.Stream";
const MAX_SOURCES: usize = 16;
const FRAME_BYTES: usize = 16 * 1024;
const RESPONSE_BYTES: usize = 32 * 1024;
const CALL_TIME: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Source {
    Monitor(Monitor),
    Window(u64),
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Cursor {
    Hidden,
    Embedded,
    Metadata,
}
impl Cursor {
    fn compositor_value(self) -> u32 {
        match self {
            Self::Hidden => 0,
            Self::Embedded => 1,
            Self::Metadata => 2,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stream {
    pub node: u32,
    pub source: Source,
    // Compositor coordinates, not the stream's physical buffer dimensions.
    pub geometry: Option<Geometry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mapping: Option<Mapping>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Geometry {
    pub position: (i32, i32),
    pub size: (i32, i32),
}
impl Geometry {
    fn valid(self) -> bool {
        (1..=32_768).contains(&self.size.0)
            && (1..=32_768).contains(&self.size.1)
            && self.position.0.abs_diff(0) <= 1_000_000
            && self.position.1.abs_diff(0) <= 1_000_000
    }
}

#[derive(Clone, Debug)]
pub struct Owner(OwnedUniqueName);
impl Owner {
    pub async fn process_id(&self, connection: &Connection) -> anyhow::Result<u32> {
        Ok(tokio::time::timeout(CALL_TIME, async {
            zbus::fdo::DBusProxy::new(connection)
                .await?
                .get_connection_unix_process_id(self.0.clone().into())
                .await
        })
        .await??)
    }
    /// Resolve without activating a service, and verify the pinned endpoint.
    pub async fn resolve(connection: &Connection) -> anyhow::Result<Self> {
        let owner = tokio::time::timeout(CALL_TIME, async {
            zbus::fdo::DBusProxy::new(connection)
                .await?
                .get_name_owner(SERVICE.try_into().expect("service name"))
                .await
        })
        .await??;
        let version: zbus::zvariant::OwnedValue = worker::call(
            connection,
            owner.as_str(),
            PATH,
            "org.freedesktop.DBus.Properties",
            "Get",
            &(SERVICE, "Version"),
            CALL_TIME,
        )
        .await?;
        anyhow::ensure!(i32::try_from(version)? >= 4, "Unsupported capture service");
        require_owner_cleanup(connection, owner.as_str()).await?;
        Ok(Self(owner))
    }
}

async fn require_owner_cleanup(connection: &Connection, owner: &str) -> anyhow::Result<()> {
    let version: zbus::zvariant::OwnedValue = worker::call(
        connection,
        owner,
        PATH,
        "org.freedesktop.DBus.Properties",
        "Get",
        &(SERVICE, "CanopaCaptureOwnerVersion"),
        CALL_TIME,
    )
    .await?;
    anyhow::ensure!(
        u32::try_from(version)? == 1,
        "Capture creator cleanup unavailable"
    );
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Input {
    compositor: String,
    backend: String,
    caller: String,
    sources: Vec<Source>,
    cursor: Cursor,
}
impl Input {
    fn validate(&self) -> anyhow::Result<()> {
        for owner in [&self.compositor, &self.backend, &self.caller] {
            let _: zbus::names::UniqueName<'_> = owner.as_str().try_into()?;
        }
        anyhow::ensure!(
            !self.sources.is_empty() && self.sources.len() <= MAX_SOURCES,
            "Invalid capture sources"
        );
        let mut seen = HashSet::new();
        for source in &self.sources {
            let target = match source {
                Source::Monitor(monitor) => (Some(monitor.name()), 0),
                Source::Window(id) => (None, *id),
            };
            anyhow::ensure!(seen.insert(target), "Duplicate capture source");
            anyhow::ensure!(
                match source {
                    Source::Monitor(monitor) => monitor.valid(),
                    Source::Window(id) => *id != 0,
                },
                "Invalid capture source"
            );
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct Recorder {
    process: Arc<DialogProcess>,
    slots: Arc<Semaphore>,
    sources: Arc<Semaphore>,
}
impl Recorder {
    pub fn new(process: Arc<DialogProcess>) -> Self {
        Self {
            process,
            slots: Arc::new(Semaphore::new(8)),
            sources: Arc::new(Semaphore::new(MAX_SOURCES)),
        }
    }
    /// Call only after explicit source consent. Dropping a pending start retires it.
    pub async fn start(
        &self,
        connection: &Connection,
        owner: &Owner,
        caller: OwnedUniqueName,
        sources: Vec<Source>,
        cursor: Cursor,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Recording> {
        let input = Input {
            compositor: owner.0.to_string(),
            backend: connection
                .unique_name()
                .ok_or_else(|| anyhow::anyhow!("Capture connection is not on a bus"))?
                .to_string(),
            caller: caller.to_string(),
            sources,
            cursor,
        };
        input.validate()?;
        let permit = self.slots.clone().try_acquire_owned()?;
        let sources = self
            .sources
            .clone()
            .try_acquire_many_owned(input.sources.len() as u32)?;
        transport::start(&self.process, input, cancelled, (permit, sources)).await
    }
}

pub struct Recording {
    streams: Vec<Stream>,
    bindings: Option<Bindings>,
    lease: Option<oneshot::Sender<()>>,
    closed: watch::Receiver<bool>,
    done: oneshot::Receiver<anyhow::Result<()>>,
}
impl Recording {
    pub fn streams(&self) -> &[Stream] {
        &self.streams
    }
    /// Only complete, identity-checked mappings may authorize absolute input.
    pub fn bindings(&self) -> Option<&Bindings> {
        self.bindings.as_ref()
    }
    /// Becomes true after the worker exits, including unexpected source loss.
    pub fn closed(&self) -> watch::Receiver<bool> {
        self.closed.clone()
    }
    pub async fn stop(mut self) -> anyhow::Result<()> {
        self.lease.take();
        (&mut self.done).await??;
        Ok(())
    }
}
impl Drop for Recording {
    fn drop(&mut self) {
        // The detached reaper keeps admission until this worker actually exits.
        self.lease.take();
    }
}

/// Private worker entry point; never registers a portal capability.
pub fn run_worker() -> anyhow::Result<()> {
    worker::stdio()
}
