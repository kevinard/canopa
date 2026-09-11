//! Freedesktop wire contract, live identity, and authoritative signal publication.
use canopa_notify::{Daemon, NotificationServerRequest, SenderEvent};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::OwnedValue;

mod ids;
use ids::{IdError, NotificationSenderIds, SenderNotificationId};

#[cfg(test)]
mod tests;

struct NotificationServer {
    daemon: Daemon,
    sender_ids: NotificationSenderIds,
}

const IMPLEMENTED_SPECIFICATION_CAPABILITIES: &[&str] = &[
    "actions",
    "action-icons",
    "body",
    "body-markup",
    "icon-static",
];
const ACTIVATION_TOKEN_EXTENSION_CAPABILITY: &str = "activation-token";

#[derive(Clone, Debug, PartialEq, Eq)]
struct CapabilityAdvertisement {
    specification: Vec<&'static str>,
    extensions: Vec<&'static str>,
}

impl CapabilityAdvertisement {
    fn for_runtime(sound: bool, activation_token: bool) -> Self {
        let mut specification = IMPLEMENTED_SPECIFICATION_CAPABILITIES.to_vec();
        if sound {
            specification.push("sound");
        }

        let mut extensions = Vec::new();
        if activation_token {
            extensions.push(ACTIVATION_TOKEN_EXTENSION_CAPABILITY);
        }

        Self {
            specification,
            extensions,
        }
    }

    fn into_dbus_values(self) -> Vec<String> {
        self.specification
            .into_iter()
            .chain(self.extensions)
            .map(str::to_string)
            .collect()
    }
}

fn capability_advertisement(daemon: &Daemon) -> CapabilityAdvertisement {
    CapabilityAdvertisement::for_runtime(
        daemon.supports_sound(),
        daemon.supports_activation_tokens(),
    )
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> zbus::fdo::Result<u32> {
        let request = NotificationServerRequest {
            app_name,
            replaces_id,
            app_icon,
            summary,
            body,
            actions,
            hints,
            expire_timeout,
        };
        let mut input = request
            .into_notification_input()
            .map_err(|error| zbus::fdo::Error::LimitsExceeded(error.to_string()))?;
        let resolved = std::sync::OnceLock::new();

        self.daemon
            .receive_notification_with_preparation(
                || {
                    let sender_id = self.sender_ids.prepare(replaces_id)?;
                    // A wire ID is never a SQLite row ID. Resolve under the mutation boundary.
                    input.replaces_id = sender_id.replacement_target;
                    resolved.set(sender_id).unwrap();
                    Ok(input)
                },
                |receipt| {
                    self.sender_ids
                        .commit(*resolved.get().unwrap(), receipt.notification_id);
                },
            )
            .await
            .map_err(|error| match error.downcast_ref::<IdError>() {
                Some(IdError::Retired) => zbus::fdo::Error::InvalidArgs(error.to_string()),
                Some(IdError::Exhausted | IdError::Fragmented) => {
                    zbus::fdo::Error::LimitsExceeded(error.to_string())
                }
                None if error.is::<canopa_notify::NotificationLimitExceeded>() => {
                    zbus::fdo::Error::LimitsExceeded(error.to_string())
                }
                None => zbus::fdo::Error::Failed(error.to_string()),
            })?;

        Ok(resolved.into_inner().unwrap().value.get())
    }

    async fn close_notification(&self, id: u32) -> zbus::fdo::Result<()> {
        let Some(internal_id) = self.sender_ids.internal_id(id) else {
            return Err(empty_dbus_error());
        };

        self.daemon
            .close_notification(internal_id)
            .await
            .map_err(|_error| empty_dbus_error())
    }

    // An injected header makes zbus validate the empty argument list. Without any
    // parameters its generated dispatcher skips body deserialization entirely.
    fn get_capabilities(&self, #[zbus(header)] _header: zbus::message::Header<'_>) -> Vec<String> {
        capability_advertisement(&self.daemon).into_dbus_values()
    }

    fn get_server_information(
        &self,
        #[zbus(header)] _header: zbus::message::Header<'_>,
    ) -> (String, String, String, String) {
        (
            "Canopa Notify".to_string(),
            "Canopa".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
            "1.3".to_string(),
        )
    }

    #[zbus(signal)]
    async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn activation_token(
        emitter: &SignalEmitter<'_>,
        id: u32,
        activation_token: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;
}

#[cfg(test)]
async fn start_notification_server(
    bus_name: String,
    daemon: Daemon,
) -> anyhow::Result<zbus::Connection> {
    let connection = zbus::connection::Builder::session()?
        .name(bus_name)?
        .build()
        .await?;
    attach_notification_server(connection.clone(), daemon).await?;
    Ok(connection)
}

pub(crate) async fn attach_notification_server(
    connection: zbus::Connection,
    daemon: Daemon,
) -> anyhow::Result<()> {
    attach_with_ids(connection, daemon, NotificationSenderIds::new()).await
}

async fn attach_with_ids(
    connection: zbus::Connection,
    daemon: Daemon,
    sender_ids: NotificationSenderIds,
) -> anyhow::Result<()> {
    daemon.install_sender_event_publisher(DbusSenderPublisher::start(
        connection.clone(),
        sender_ids.clone(),
    )?)?;
    connection
        .object_server()
        .at(
            "/org/freedesktop/Notifications",
            NotificationServer { daemon, sender_ids },
        )
        .await?;
    Ok(())
}

struct DbusSenderPublisher {
    queue: tokio::sync::mpsc::Sender<(SenderNotificationId, SenderEvent)>,
    sender_ids: NotificationSenderIds,
    error: Arc<Mutex<Option<String>>>,
}

impl DbusSenderPublisher {
    fn start(
        connection: zbus::Connection,
        sender_ids: NotificationSenderIds,
    ) -> anyhow::Result<Self> {
        let emitter =
            SignalEmitter::new(&connection, "/org/freedesktop/Notifications")?.into_owned();
        let (queue, mut events) = tokio::sync::mpsc::channel(64);
        let error = Arc::new(Mutex::new(None));
        let transport_error = error.clone();
        tokio::spawn(async move {
            while let Some((sender_id, event)) = events.recv().await {
                if let Err(error) = emit_sender_event(&emitter, sender_id, &event).await {
                    *transport_error.lock().unwrap() = Some(format!("{error:#}"));
                    eprintln!("level=error component=dbus event=sender_signal_failed");
                    break;
                }
            }
        });
        Ok(Self {
            queue,
            sender_ids,
            error,
        })
    }
}

impl canopa_notify::SenderEventPublisher for DbusSenderPublisher {
    fn publish(&self, event: SenderEvent) -> canopa_notify::SenderEventPublication {
        // Freeze identity at commitment, before waiting for transport capacity.
        // A failed or delayed signal cannot keep a closed registration alive.
        let sender_id = match &event {
            SenderEvent::NotificationClosed {
                notification_id, ..
            } => self.sender_ids.retire(*notification_id),
            SenderEvent::ActivationToken {
                notification_id, ..
            }
            | SenderEvent::ActionInvoked {
                notification_id, ..
            } => self.sender_ids.sender_id(*notification_id),
        };
        let queue = self.queue.clone();
        Box::pin(async move {
            if let Some(sender_id) = sender_id {
                queue
                    .send((sender_id, event))
                    .await
                    .map_err(|_| anyhow::anyhow!("D-Bus sender publisher disconnected"))?;
            }
            Ok(())
        })
    }

    fn error(&self) -> Option<String> {
        self.error.lock().unwrap().clone()
    }
}

async fn emit_sender_event(
    emitter: &SignalEmitter<'_>,
    sender_id: SenderNotificationId,
    event: &SenderEvent,
) -> zbus::Result<()> {
    let sender_id = sender_id.get();
    match event {
        SenderEvent::ActivationToken {
            activation_token, ..
        } => {
            emitter
                .activation_token(sender_id, activation_token)
                .await?;
        }
        SenderEvent::ActionInvoked { action_key, .. } => {
            emitter.action_invoked(sender_id, action_key).await?;
        }
        SenderEvent::NotificationClosed { reason, .. } => {
            emitter
                .notification_closed(sender_id, close_reason_code(*reason))
                .await?;
        }
    }

    Ok(())
}

fn empty_dbus_error() -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(String::new())
}

fn close_reason_code(reason: canopa_notify::SenderCloseReason) -> u32 {
    match reason {
        canopa_notify::SenderCloseReason::Expired => 1,
        canopa_notify::SenderCloseReason::Dismissed => 2,
        canopa_notify::SenderCloseReason::Closed => 3,
        canopa_notify::SenderCloseReason::Undefined => 4,
    }
}
