use super::host::*;
use crate::test_bus::TestBus;
use std::sync::{Arc, Mutex};

struct Item(Arc<Mutex<String>>);
#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl Item {
    #[zbus(property)]
    fn icon_name(&self) -> String {
        self.0.lock().unwrap().clone()
    }
    #[zbus(property)]
    fn status(&self) -> &str {
        "Active"
    }
}

#[tokio::test]
async fn item_changes_repeat_and_owner_loss_removes_path_registration() {
    let bus = TestBus::new();
    let host = bus.connect().await;
    let item = bus.connect().await;
    let icon = Arc::new(Mutex::new("first".into()));
    item.object_server()
        .at("/CustomItem", Item(icon.clone()))
        .await
        .unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let task = tokio::spawn(run_sni_task(host.clone(), tx));
    async fn receive(rx: &mut tokio::sync::mpsc::UnboundedReceiver<TrayEvent>) -> TrayEvent {
        tokio::time::timeout(std::time::Duration::from_secs(3), rx.recv())
            .await
            .unwrap()
            .unwrap()
    }
    assert!(matches!(receive(&mut rx).await, TrayEvent::Snapshot(_)));
    let watcher = zbus::Proxy::new(
        &item,
        "org.kde.StatusNotifierWatcher",
        "/StatusNotifierWatcher",
        "org.kde.StatusNotifierWatcher",
    )
    .await
    .unwrap();
    watcher
        .call::<_, _, ()>("RegisterStatusNotifierItem", &("/CustomItem",))
        .await
        .unwrap();
    let TrayEvent::ItemAdded(registered) = receive(&mut rx).await else {
        panic!("registration")
    };
    let identity = registered.runtime_id();
    for name in ["second", "third"] {
        *icon.lock().unwrap() = name.into();
        item.emit_signal(
            None::<&str>,
            "/CustomItem",
            "org.kde.StatusNotifierItem",
            "NewIcon",
            &(),
        )
        .await
        .unwrap();
        let TrayEvent::ItemAdded(updated) = receive(&mut rx).await else {
            panic!("updated icon")
        };
        assert!(matches!(updated.icon, TrayIcon::Named(ref icon) if icon == name));
    }
    drop(watcher);
    item.close().await.unwrap();
    assert!(matches!(receive(&mut rx).await, TrayEvent::ItemRemoved(id) if id == identity));
    let observer = super::proxy::SniWatcherProxy::new(&host).await.unwrap();
    // The host event can precede its watcher bookkeeping by one scheduler turn.
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if observer
                .registered_status_notifier_items()
                .await
                .unwrap()
                .is_empty()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    drop(rx);
    task.await.unwrap();
}
