//! Credentials are scoped to one connection attempt and one daemon owner.
use super::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "net.connman.iwd.Agent.Error")]
enum AgentError {
    Canceled,
}

struct Agent {
    conn: zbus::Connection,
    owner: String,
    kind: String,
    station: OwnedObjectPath,
    label: String,
    secret: Arc<Mutex<Option<String>>>,
}

#[zbus::interface(name = "net.connman.iwd.Agent")]
impl Agent {
    async fn request_passphrase(
        &self,
        network: OwnedObjectPath,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<String, AgentError> {
        if header.sender().map(|s| s.as_str()) != Some(self.owner.as_str()) {
            return Err(AgentError::Canceled);
        }
        verify_control(&self.conn, crate::network::bundle::Kind::Iwd, &self.owner)
            .await
            .map_err(|_| AgentError::Canceled)?;
        let proxy = IwdNetworkProxy::builder(&self.conn)
            .destination(self.owner.clone())
            .map_err(|_| AgentError::Canceled)?
            .path(network)
            .map_err(|_| AgentError::Canceled)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .map_err(|_| AgentError::Canceled)?;
        if proxy.name().await.ok().as_deref() != Some(&self.label)
            || proxy.network_type().await.ok().as_deref() != Some(&self.kind)
            || proxy.device().await.ok().as_ref() != Some(&self.station)
        {
            return Err(AgentError::Canceled);
        }
        verify_control(&self.conn, crate::network::bundle::Kind::Iwd, &self.owner)
            .await
            .map_err(|_| AgentError::Canceled)?;
        self.secret
            .lock()
            .unwrap()
            .clone()
            .ok_or(AgentError::Canceled)
    }

    fn release(&self, #[zbus(header)] header: zbus::message::Header<'_>) {
        if header.sender().map(|s| s.as_str()) == Some(self.owner.as_str()) {
            self.secret.lock().unwrap().take();
        }
    }

    fn cancel(&self, _reason: &str, #[zbus(header)] header: zbus::message::Header<'_>) {
        self.release(header);
    }
}

#[zbus::proxy(
    interface = "net.connman.iwd.AgentManager",
    default_service = "net.connman.iwd",
    default_path = "/net/connman/iwd"
)]
trait AgentManager {
    fn register_agent(&self, path: &OwnedObjectPath) -> zbus::Result<()>;
    fn unregister_agent(&self, path: &OwnedObjectPath) -> zbus::Result<()>;
}

pub(crate) struct IwdAgentRegistration {
    cleanup: cleanup::Cleanup,
    conn: zbus::Connection,
    path: OwnedObjectPath,
    owner: String,
    secret: Arc<Mutex<Option<String>>>,
}

impl IwdAgentRegistration {
    /// Answer `owner`'s passphrase request for `label` of type `kind` on
    /// `station` with `credential`, until dropped.
    pub(crate) async fn register(
        conn: &zbus::Connection,
        owner: &str,
        credential: Option<String>,
        station: &OwnedObjectPath,
        label: &str,
        kind: &str,
        cleanup: cleanup::Cleanup,
    ) -> Result<Self, WifiBackendError> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = OwnedObjectPath::try_from(format!(
            "/org/canopa/WifiAgent/attempt{}",
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
        .unwrap();
        let registration = Self {
            cleanup,
            conn: conn.clone(),
            owner: owner.into(),
            path,
            secret: Arc::new(Mutex::new(credential)),
        };
        conn.object_server()
            .at(
                registration.path.clone(),
                Agent {
                    conn: conn.clone(),
                    owner: owner.into(),
                    kind: kind.into(),
                    station: station.clone(),
                    label: label.into(),
                    secret: registration.secret.clone(),
                },
            )
            .await
            .map_err(connection_error)?;
        AgentManagerProxy::builder(conn)
            .destination(owner)
            .map_err(connection_error)?
            .build()
            .await
            .map_err(connection_error)?
            .register_agent(&registration.path)
            .await
            .map_err(connection_error)?;
        Ok(registration)
    }
}

impl Drop for IwdAgentRegistration {
    fn drop(&mut self) {
        self.secret.lock().unwrap().take();
        let conn = self.conn.clone();
        let path = self.path.clone();
        let owner = self.owner.clone();
        self.cleanup.spawn(async move {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                if let Ok(manager) = AgentManagerProxy::builder(&conn)
                    .destination(owner)
                    .unwrap()
                    .build()
                    .await
                {
                    let _ = manager.unregister_agent(&path).await;
                }
            })
            .await;
            let _ = conn.object_server().remove::<Agent, _>(path).await;
        });
    }
}
