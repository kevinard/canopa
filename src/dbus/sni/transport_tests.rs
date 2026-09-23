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

struct MenuOnlyItem(tokio::sync::mpsc::UnboundedSender<&'static str>);
#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl MenuOnlyItem {
    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn icon_name(&self) -> &str {
        "menu"
    }
    #[zbus(property)]
    fn status(&self) -> &str {
        "Active"
    }
    fn activate(&self, _x: i32, _y: i32) {
        self.0.send("activate").unwrap();
    }
    fn context_menu(&self, _x: i32, _y: i32) {
        self.0.send("menu").unwrap();
    }
}

#[tokio::test]
async fn menu_only_item_without_exported_menu_uses_context_menu() {
    let bus = TestBus::new();
    let service = bus.connect().await;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    service
        .object_server()
        .at("/Item", MenuOnlyItem(tx))
        .await
        .unwrap();
    let client = bus.connect().await;
    let name = service.unique_name().unwrap().as_str();
    let item = fetch_item(&client, name, &"/Item".try_into().unwrap())
        .await
        .unwrap();
    assert!(item.item_is_menu);
    assert!(item.menu_path.is_none());
    call_activate(&client, name, &item.object_path)
        .await
        .unwrap();
    assert_eq!(rx.recv().await.unwrap(), "menu");
    assert!(rx.try_recv().is_err());
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
