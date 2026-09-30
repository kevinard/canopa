// StatusNotifierWatcher implementation — Task 3.
use std::sync::Arc;
use tokio::sync::Mutex;
use zbus::{interface, object_server::SignalEmitter};

#[derive(Default)]
struct StatusNotifierWatcher {
    items: Arc<Mutex<Vec<String>>>,
    hosts: Arc<Mutex<Vec<String>>>,
}

impl StatusNotifierWatcher {
    fn new() -> Self {
        StatusNotifierWatcher::default()
    }
}

#[interface(name = "org.kde.StatusNotifierWatcher")]
impl StatusNotifierWatcher {
    async fn register_status_notifier_item(
        &self,
        service: &str,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        let full = if service.starts_with('/') {
            let sender = header
                .sender()
                .ok_or_else(|| zbus::fdo::Error::InvalidArgs("no sender".into()))?;
            format!("{}{}", sender, service)
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
            drop(hosts);
            Self::status_notifier_host_registered(&emitter).await?;
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
    async fn registered_status_notifier_items(&self) -> Vec<String> {
        self.items.lock().await.clone()
    }

    #[zbus(property)]
    async fn is_status_notifier_host_registered(&self) -> bool {
        !self.hosts.lock().await.is_empty()
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
pub(super) async fn start_watcher(
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
        Err(e) => {
            let _ = conn
                .object_server()
                .remove::<StatusNotifierWatcher, _>("/StatusNotifierWatcher")
                .await;
            Err(e)
        }
    }
}

/// Retire every object owned by a disappearing bus name, including path registrations.
pub(super) async fn remove_owner(conn: &zbus::Connection, owner: &str) -> zbus::Result<()> {
    let interface = conn
        .object_server()
        .interface::<_, StatusNotifierWatcher>("/StatusNotifierWatcher")
        .await?;
    let watcher = interface.get().await;
    let mut items = watcher.items.lock().await;
    let removed: Vec<_> = items
        .iter()
        .filter(|item| item.split('/').next() == Some(owner))
        .cloned()
        .collect();
    items.retain(|item| !removed.contains(item));
    drop(items);
    watcher.hosts.lock().await.retain(|host| host != owner);
    for item in removed {
        StatusNotifierWatcher::status_notifier_item_unregistered(interface.signal_emitter(), &item)
            .await?;
    }
    Ok(())
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
