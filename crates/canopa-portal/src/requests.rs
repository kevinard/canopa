//! Shared interactive-request ownership, admission and retirement.
use futures_util::StreamExt;
use std::{collections::HashMap, future::Future, sync::Arc};
use tokio::sync::{Semaphore, watch};
use zbus::{
    Connection, fdo,
    message::Header,
    names::OwnedUniqueName,
    zvariant::{OwnedObjectPath, OwnedValue},
};

pub type Options = HashMap<String, OwnedValue>;
// Keep this alias inside Rust boundaries. zbus's macro needs the literal tuple
// on published method return types to derive two separate introspection outputs.
pub type Response = (u32, Options);

pub(crate) fn option<T: TryFrom<OwnedValue>>(
    options: &mut Options,
    key: &str,
) -> fdo::Result<Option<T>> {
    options
        .remove(key)
        .map(|value| {
            T::try_from(value).map_err(|_| fdo::Error::InvalidArgs(format!("Invalid {key}")))
        })
        .transpose()
}

#[derive(Clone)]
pub struct Requests {
    slots: Arc<Semaphore>,
}

impl Default for Requests {
    fn default() -> Self {
        Self {
            slots: Arc::new(Semaphore::new(16)),
        }
    }
}

pub(crate) enum Completion<T> {
    Cancelled,
    Finished(T),
}

impl Requests {
    pub(crate) async fn run<T, F, Pending>(
        &self,
        connection: &Connection,
        header: Header<'_>,
        handle: OwnedObjectPath,
        interaction: F,
    ) -> fdo::Result<Completion<T>>
    where
        T: Send,
        F: FnOnce(OwnedUniqueName, watch::Receiver<bool>) -> Pending + Send,
        Pending: Future<Output = T> + Send,
    {
        self.run_owned(connection, header, handle, false, interaction)
            .await
    }

    /// The frontend monitor uses /notify/backgroundN rather than /request/….
    /// Keep this extra namespace confined to Background, sharing owner checks,
    /// capacity and retirement with every other interactive request.
    pub(crate) async fn run_background<T, F, Pending>(
        &self,
        connection: &Connection,
        header: Header<'_>,
        handle: OwnedObjectPath,
        interaction: F,
    ) -> fdo::Result<Completion<T>>
    where
        T: Send,
        F: FnOnce(OwnedUniqueName, watch::Receiver<bool>) -> Pending + Send,
        Pending: Future<Output = T> + Send,
    {
        self.run_owned(connection, header, handle, true, interaction)
            .await
    }

    async fn run_owned<T, F, Pending>(
        &self,
        connection: &Connection,
        header: Header<'_>,
        handle: OwnedObjectPath,
        background: bool,
        interaction: F,
    ) -> fdo::Result<Completion<T>>
    where
        T: Send,
        F: FnOnce(OwnedUniqueName, watch::Receiver<bool>) -> Pending + Send,
        Pending: Future<Output = T> + Send,
    {
        let _slot = self
            .slots
            .try_acquire()
            .map_err(|_| fdo::Error::LimitsExceeded("Too many portal dialogs".into()))?;
        let sender: OwnedUniqueName = header
            .sender()
            .ok_or_else(|| fdo::Error::AccessDenied("Missing sender".into()))?
            .to_owned()
            .into();
        let notification = background
            && handle
                .as_str()
                .strip_prefix("/org/freedesktop/portal/desktop/notify/background")
                .is_some_and(|suffix| !suffix.is_empty() && !suffix.contains('/'));
        if !notification
            && !handle
                .as_str()
                .starts_with("/org/freedesktop/portal/desktop/request/")
        {
            return Err(fdo::Error::InvalidArgs("Invalid request path".into()));
        }
        let (cancel, cancelled) = watch::channel(false);
        let mut cancellation = cancelled.clone();
        let bus = fdo::DBusProxy::new(connection).await?;
        let mut owners = bus
            .receive_name_owner_changed_with_args(&[(0, sender.as_str())])
            .await?;
        // Subscribe first, so a disconnect during setup cannot strand a dialog.
        if !bus.name_has_owner(sender.clone().into()).await? {
            return Ok(Completion::Cancelled);
        }
        if !connection
            .object_server()
            .at(
                handle.clone(),
                Request {
                    sender: sender.clone(),
                    cancel: cancel.clone(),
                },
            )
            .await?
        {
            return Err(fdo::Error::InvalidArgs("Request already exists".into()));
        }
        let outcome = tokio::select! {
            biased;
            _ = async { if !*cancellation.borrow() { let _ = cancellation.changed().await; } } => Completion::Cancelled,
            _ = owners.next() => { cancel.send_replace(true); Completion::Cancelled },
            result = interaction(sender, cancelled) => Completion::Finished(result),
        };
        connection
            .object_server()
            .remove::<Request, _>(handle)
            .await?;
        Ok(if *cancel.borrow() {
            Completion::Cancelled
        } else {
            outcome
        })
    }
}

struct Request {
    sender: OwnedUniqueName,
    cancel: watch::Sender<bool>,
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Request")]
impl Request {
    fn close(&self, #[zbus(header)] header: Header<'_>) -> fdo::Result<()> {
        if header.sender().map(|s| s.as_str()) != Some(self.sender.as_str()) {
            return Err(fdo::Error::AccessDenied(
                "Request belongs to another caller".into(),
            ));
        }
        self.cancel.send_replace(true);
        Ok(())
    }
}
