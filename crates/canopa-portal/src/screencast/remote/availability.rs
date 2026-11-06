//! Read-only device availability notifications for the registered backend.
use super::*;
use futures_util::StreamExt;
use zbus::{MatchRule, MessageStream, message::Type, object_server::SignalEmitter};

pub struct AvailabilityWatch(Option<tokio::task::JoinHandle<()>>);
impl AvailabilityWatch {
    pub async fn stop(mut self) {
        if let Some(task) = self.0.take() {
            task.abort();
            let _ = task.await;
        }
    }
}
impl Drop for AvailabilityWatch {
    fn drop(&mut self) {
        if let Some(task) = &self.0 {
            task.abort();
        }
    }
}
async fn publish(emitter: &SignalEmitter<'_>, value: u32) -> zbus::Result<()> {
    zbus::fdo::Properties::properties_changed(
        emitter,
        "org.freedesktop.impl.portal.RemoteDesktop".try_into()?,
        HashMap::from([("AvailableDeviceTypes", Value::from(value))]),
        Default::default(),
    )
    .await
}
impl RemoteDesktop {
    /// Start after registering the interface and retain the guard until shutdown.
    /// The frontend caches this property, so fresh getters alone are insufficient.
    pub async fn observe_available_devices(
        &self,
        connection: &Connection,
    ) -> anyhow::Result<AvailabilityWatch> {
        let bus = zbus::fdo::DBusProxy::new(connection).await?;
        let mut owners = bus
            .receive_name_owner_changed_with_args(&[(0, crate::remote_input::SERVICE)])
            .await?;
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .interface("org.freedesktop.DBus.Properties")?
            .member("PropertiesChanged")?
            .path(crate::remote_input::PATH)?
            .arg(0, crate::remote_input::SERVICE)?
            .build();
        let mut properties = MessageStream::for_match_rule(rule, connection, Some(32)).await?;
        let presenter = self.methods.capture.host.remote_presenter.clone();
        let mut current = presenter.available_devices(connection).await;
        let emitter = SignalEmitter::new(connection, crate::OBJECT_PATH)?.to_owned();
        publish(&emitter, current).await?;
        let connection = connection.clone();
        Ok(AvailabilityWatch(Some(tokio::spawn(async move {
            loop {
                let owner_event = tokio::select! {
                    event = owners.next() => { if event.is_none() { let _=publish(&emitter,0).await; return; } true },
                    // A bounded stream may report a missed notification. Re-read
                    // authoritative state rather than trusting signal contents.
                    event = properties.next() => { if event.is_none() { let _=publish(&emitter,0).await; return; } false },
                };
                // Retire an old owner's cached capability before validating a
                // possible replacement. A signal payload never authorizes input.
                if owner_event && current != 0 {
                    if publish(&emitter, 0).await.is_err() {
                        return;
                    }
                    current = 0;
                }
                let next = presenter.available_devices(&connection).await;
                if next != current {
                    if publish(&emitter, next).await.is_err() {
                        return;
                    }
                    current = next;
                }
            }
        }))))
    }
}
