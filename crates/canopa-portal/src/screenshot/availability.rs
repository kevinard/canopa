use super::*;
use futures_util::StreamExt;
use std::collections::HashMap;

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
async fn publish(emitter: &zbus::object_server::SignalEmitter<'_>, value: u32) -> zbus::Result<()> {
    zbus::fdo::Properties::properties_changed(
        emitter,
        "org.freedesktop.impl.portal.Screenshot".try_into()?,
        HashMap::from([("AvailableTargets", Value::from(value))]),
        Default::default(),
    )
    .await
}
impl Screenshot {
    pub async fn observe_available_targets(
        &self,
        connection: &Connection,
    ) -> anyhow::Result<AvailabilityWatch> {
        let bus = zbus::fdo::DBusProxy::new(connection).await?;
        let mut owners = bus
            .receive_name_owner_changed_with_args(&[(0, crate::screencast_capture::SERVICE)])
            .await?;
        let presenter = self.presenter.clone();
        let mut current = presenter.available_targets(connection).await;
        let emitter =
            zbus::object_server::SignalEmitter::new(connection, crate::OBJECT_PATH)?.to_owned();
        publish(&emitter, current).await?;
        let connection = connection.clone();
        Ok(AvailabilityWatch(Some(tokio::spawn(async move {
            while owners.next().await.is_some() {
                if current != 5 {
                    if publish(&emitter, 5).await.is_err() {
                        return;
                    }
                    current = 5;
                }
                let next = presenter.available_targets(&connection).await;
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
