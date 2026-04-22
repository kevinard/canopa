use std::hash::Hash;

use iced::futures::{Stream, StreamExt};
use zbus::zvariant::OwnedObjectPath;

use super::host::{TrayEvent, run_sni_task};
use super::proxy::DbusMenuProxy;

struct SniConnData(zbus::Connection);

impl Hash for SniConnData {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        "sni-subscription".hash(h);
    }
}

fn build_sni_stream(
    data: &SniConnData,
) -> std::pin::Pin<Box<dyn Stream<Item = crate::app::Message> + Send + 'static>> {
    let conn = data.0.clone();
    crate::dbus::subscription::build::<TrayEvent, crate::app::Message, _, _, _>(
        move |tx| {
            tokio::spawn(async move {
                let _ = tx.send(Ok(TrayEvent::Snapshot(vec![])));

                let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
                tokio::spawn(async move {
                    let _ = run_sni_task(conn, event_tx).await;
                });

                while let Some(event) = event_rx.recv().await {
                    if tx.send(Ok(event)).is_err() {
                        break;
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
/// and emits `Message::TrayEvent` on every change.
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
        self.service.hash(h);
        self.menu_path.as_str().hash(h);
    }
}

enum MenuWatchState {
    Init,
    Watching,
}

fn build_menu_layout_stream(
    data: &MenuWatchKey,
) -> std::pin::Pin<Box<dyn Stream<Item = crate::app::Message> + Send + 'static>> {
    let conn = data.conn.clone();
    let service = data.service.clone();
    let menu_path = data.menu_path.clone();

    Box::pin(iced::futures::stream::unfold(
        MenuWatchState::Init,
        move |state| {
            let conn = conn.clone();
            let service = service.clone();
            let menu_path = menu_path.clone();
            async move {
                match state {
                    MenuWatchState::Init => {
                        let proxy = match DbusMenuProxy::builder(&conn)
                            .destination(service.as_str())
                            .and_then(|b| b.path(menu_path.as_ref()))
                        {
                            Ok(b) => match b.build().await {
                                Ok(p) => p,
                                Err(_) => return None,
                            },
                            Err(_) => return None,
                        };
                        let mut layout_stream = match proxy.receive_layout_updated().await {
                            Ok(s) => s,
                            Err(_) => return None,
                        };
                        let mut props_stream = match proxy.receive_items_properties_updated().await
                        {
                            Ok(s) => s,
                            Err(_) => return None,
                        };

                        tokio::select! {
                            _ = layout_stream.next() => {}
                            _ = props_stream.next() => {}
                        }
                        Some((
                            crate::app::Message::Menu(
                                crate::app::messages::MenuEvent::LayoutChanged {
                                    service: service.clone(),
                                    menu_path: menu_path.clone(),
                                },
                            ),
                            MenuWatchState::Watching,
                        ))
                    }
                    MenuWatchState::Watching => None,
                }
            }
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
