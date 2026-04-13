// StatusNotifierWatcher implementation — Task 3.
use std::sync::Arc;
use tokio::sync::Mutex;
use zbus::{interface, object_server::SignalEmitter};

#[allow(dead_code)]
pub struct StatusNotifierWatcher {
    pub items: Arc<Mutex<Vec<String>>>,
    pub hosts: Arc<Mutex<Vec<String>>>,
}

impl StatusNotifierWatcher {
    #[allow(dead_code)]
    pub fn new() -> Self {
        StatusNotifierWatcher {
            items: Arc::new(Mutex::new(Vec::new())),
            hosts: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[interface(name = "org.kde.StatusNotifierWatcher")]
impl StatusNotifierWatcher {
    async fn register_status_notifier_item(
        &self,
        service: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        let full = if service.starts_with('/') {
            service.split('/').next().unwrap_or(service).to_string()
        } else {
            service.to_string()
        };
        let mut items = self.items.lock().await;
        if !items.contains(&full) {
            items.push(full.clone());
            drop(items);
            Self::status_notifier_item_registered(&emitter, &full).await?;
        }
        Ok(())
    }

    async fn register_status_notifier_host(
        &self,
        service: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        let mut hosts = self.hosts.lock().await;
        if !hosts.contains(&service.to_string()) {
            hosts.push(service.to_string());
            let is_first = hosts.len() == 1;
            drop(hosts);
            if is_first {
                Self::status_notifier_host_registered(&emitter).await?;
            }
        }
        Ok(())
    }

    #[zbus(signal)]
    async fn status_notifier_item_registered(
        emitter: &SignalEmitter<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_item_unregistered(
        emitter: &SignalEmitter<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_host_registered(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.items.blocking_lock().clone()
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        !self.hosts.blocking_lock().is_empty()
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

/// Registers the watcher on `conn`.
///
/// Requests the well-known name `org.kde.StatusNotifierWatcher`.
/// If that name is already taken (another watcher is running), returns `Ok(None)` —
/// the caller falls back to host-only mode.
///
/// On success, returns `Some(items_handle)` — the shared item list.
#[allow(dead_code)]
pub async fn start_watcher(
    conn: &zbus::Connection,
) -> zbus::Result<Option<Arc<Mutex<Vec<String>>>>> {
    let watcher = StatusNotifierWatcher::new();
    let items = watcher.items.clone();

    conn.object_server()
        .at("/StatusNotifierWatcher", watcher)
        .await?;

    match conn.request_name("org.kde.StatusNotifierWatcher").await {
        Ok(()) => Ok(Some(items)),
        Err(zbus::Error::NameTaken) => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watcher_starts_with_no_items() {
        let w = StatusNotifierWatcher::new();
        assert!(w.items.blocking_lock().is_empty());
    }

    #[test]
    fn watcher_starts_with_no_hosts() {
        let w = StatusNotifierWatcher::new();
        assert!(w.hosts.blocking_lock().is_empty());
    }
}
