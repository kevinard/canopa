use std::hash::Hash;

use iced::futures::{Stream, StreamExt};
use zbus::zvariant::OwnedObjectPath;

use super::host::{TrayEvent, run_sni_task};
use super::proxy::DbusMenuProxy;

struct SniConnData(zbus::Connection);

impl Hash for SniConnData {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        "sni-subscription".hash(h);
        self.0.unique_name().hash(h);
        self.0.server_guid().hash(h);
    }
}

fn build_sni_stream(
    data: &SniConnData,
) -> std::pin::Pin<Box<dyn Stream<Item = crate::app::Message> + Send + 'static>> {
    let conn = data.0.clone();
    crate::dbus::subscription::build::<TrayEvent, crate::app::Message, _, _, _>(
        move |tx| {
            let conn = conn.clone();
            tokio::spawn(async move {
                let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
                let work = run_sni_task(conn, event_tx);
                tokio::pin!(work);
                loop {
                    tokio::select! {
                        _ = tx.closed() => return,
                        _ = &mut work => return,
                        event = event_rx.recv() => match event {
                            Some(event) => { if tx.send(Ok(event)).is_err() { return; } }
                            None => return,
                        }
                    }
                }
            });
        },
        |event| crate::app::Message::Tray(crate::app::messages::TrayRuntimeEvent::Host(event)),
        || {
            crate::app::Message::Tray(crate::app::messages::TrayRuntimeEvent::Host(
                TrayEvent::Snapshot(vec![]),
            ))
        },
    )
}

/// Returns an Iced subscription that starts the SNI watcher+host on `conn`
/// and emits `Message::Tray(messages::TrayRuntimeEvent::Host(_))` on every change.
pub(crate) fn sni_subscription(conn: zbus::Connection) -> iced::Subscription<crate::app::Message> {
    iced::Subscription::run_with(SniConnData(conn), build_sni_stream)
}

/// Key for the menu layout watcher subscription, ensuring it is unique per
/// (service, menu_path) pair.
struct MenuWatchKey {
    conn: zbus::Connection,
    service: String,
    menu_path: OwnedObjectPath,
}

impl Hash for MenuWatchKey {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        "menu-layout-watcher".hash(h);
        self.conn.unique_name().hash(h);
        self.conn.server_guid().hash(h);
        self.service.hash(h);
        self.menu_path.as_str().hash(h);
    }
}

fn build_menu_layout_stream(
    data: &MenuWatchKey,
) -> std::pin::Pin<Box<dyn Stream<Item = crate::app::Message> + Send + 'static>> {
    let conn = data.conn.clone();
    let service = data.service.clone();
    let menu_path = data.menu_path.clone();
    // The stream itself owns the proxy subscription: cancellation releases it.
    Box::pin(iced::futures::stream::unfold(
        (
            conn,
            service,
            menu_path,
            None::<zbus::proxy::SignalStream<'static>>,
        ),
        |(conn, service, menu_path, mut signals)| async move {
            if signals.is_none() {
                let proxy = DbusMenuProxy::builder(&conn)
                    .destination(service.clone())
                    .ok()?
                    .path(menu_path.clone())
                    .ok()?
                    .build()
                    .await
                    .ok()?;
                signals = Some(proxy.inner().receive_all_signals().await.ok()?);
            }
            let stream = signals.as_mut()?;
            while let Some(signal) = stream.next().await {
                if signal.header().member().is_some_and(|member| {
                    matches!(member.as_str(), "LayoutUpdated" | "ItemsPropertiesUpdated")
                }) {
                    let message =
                        crate::app::Message::Menu(crate::app::messages::MenuEvent::LayoutChanged {
                            service: service.clone(),
                            menu_path: menu_path.clone(),
                        });
                    return Some((message, (conn, service, menu_path, signals)));
                }
            }
            None
        },
    ))
}

/// Returns an iced Subscription that listens for `LayoutUpdated` and
/// `ItemsPropertiesUpdated` signals on the given DBusMenu path.
/// Emits `Message::Menu(MenuEvent::LayoutChanged)` when the layout changes.
pub(crate) fn menu_layout_subscription(
    conn: zbus::Connection,
    service: String,
    menu_path: OwnedObjectPath,
) -> iced::Subscription<crate::app::Message> {
    iced::Subscription::run_with(
        MenuWatchKey {
            conn,
            service,
            menu_path,
        },
        build_menu_layout_stream,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn menu_watcher_delivers_more_than_one_layout_and_property_change() {
        let bus = crate::test_bus::TestBus::new();
        let service = bus.connect().await;
        let client = bus.connect().await;
        let key = MenuWatchKey {
            conn: client,
            service: service.unique_name().unwrap().to_string(),
            menu_path: "/Menu".try_into().unwrap(),
        };
        let mut stream = build_menu_layout_stream(&key);
        for member in ["LayoutUpdated", "LayoutUpdated", "ItemsPropertiesUpdated"] {
            // Polling establishes the match rule. Repeated emission also avoids
            // relying on scheduler timing during that initial subscription.
            let emit = async {
                loop {
                    service
                        .emit_signal(None::<&str>, "/Menu", "com.canonical.dbusmenu", member, &())
                        .await
                        .unwrap();
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
            };
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                tokio::select! {
                    message = stream.next() => assert!(matches!(message, Some(crate::app::Message::Menu(crate::app::messages::MenuEvent::LayoutChanged { .. })))),
                    _ = emit => unreachable!(),
                }
            }).await.unwrap();
        }
    }
}
