//! Original-client identity is trusted only when supplied by the current frontend.
use futures_util::{FutureExt, StreamExt, future::BoxFuture};
use std::{sync::Arc, time::Duration};
use tokio::sync::{mpsc, oneshot, watch};
use zbus::{
    Connection, fdo,
    names::{BusName, OwnedUniqueName},
    zvariant::OwnedObjectPath,
};

const FRONTEND: &str = "org.freedesktop.portal.Desktop";
const REQUEST_PREFIX: &str = "/org/freedesktop/portal/desktop/request/";
const BUS_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Eq, PartialEq)]
struct Identity {
    bus: String,
    peer: OwnedUniqueName,
    client: OwnedUniqueName,
    app_id: String,
    frontend: bool,
}

struct Inner {
    identity: Identity,
    connection: Connection,
    revoked: watch::Sender<bool>,
    synchronize: mpsc::Sender<oneshot::Sender<()>>,
    monitor: tokio::task::JoinHandle<()>,
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.monitor.abort();
    }
}

/// A live, non-serializable identity proof, retained with the prepared settings.
/// Dropping the last clone retires its bus subscriptions and monitor.
#[derive(Clone)]
pub struct Owner(Arc<Inner>);

fn original_client(handle: &OwnedObjectPath) -> fdo::Result<OwnedUniqueName> {
    let mut parts = handle
        .as_str()
        .strip_prefix(REQUEST_PREFIX)
        .ok_or_else(|| super::invalid("Invalid print request path"))?
        .split('/');
    let encoded = parts.next().unwrap_or_default();
    // The canonical frontend appends a second token when resolving a collision.
    let tokens = parts.collect::<Vec<_>>();
    if encoded.is_empty()
        || !(1..=2).contains(&tokens.len())
        || tokens.iter().any(|part| part.is_empty())
    {
        return Err(super::invalid("Invalid print request identity"));
    }
    format!(":{}", encoded.replace('_', "."))
        .try_into()
        .map_err(|_| super::invalid("Invalid print request identity"))
}

fn changed(mut stream: fdo::NameOwnerChangedStream) -> BoxFuture<'static, ()> {
    async move {
        // A closed signal stream also revokes authority. Never resume a lease
        // after a name is lost and later acquired by the same connection.
        let _ = stream.next().await;
    }
    .boxed()
}

impl Owner {
    /// `peer` must come from the incoming method header, never its body.
    /// Subscribe before querying each owner to cover disconnects during setup.
    pub async fn authenticate(
        connection: &Connection,
        peer: OwnedUniqueName,
        handle: &OwnedObjectPath,
        app_id: &str,
    ) -> fdo::Result<Self> {
        if app_id.len() > 256 || app_id.contains('\0') {
            return Err(super::invalid("Invalid print application ID"));
        }
        tokio::time::timeout(BUS_TIMEOUT, async {
            let bus = fdo::DBusProxy::new(connection).await?;
            let frontend_changes = bus
                .receive_name_owner_changed_with_args(&[(0, FRONTEND)])
                .await?;
            let frontend = match bus
                .get_name_owner(BusName::try_from(FRONTEND).expect("static frontend name"))
                .await
            {
                Ok(owner) => owner == peer,
                Err(fdo::Error::NameHasNoOwner(_)) => false,
                Err(error) => return Err(error),
            };
            let client = if frontend {
                original_client(handle)?
            } else {
                // Untrusted direct calls cannot authenticate the sender encoded
                // in their handle or share tokens just by supplying an app ID.
                peer.clone()
            };
            let mut monitors = vec![changed(
                bus.receive_name_owner_changed_with_args(&[(0, peer.as_str())])
                    .await?,
            )];
            if frontend {
                monitors.push(changed(frontend_changes));
            }
            if client != peer {
                monitors.push(changed(
                    bus.receive_name_owner_changed_with_args(&[(0, client.as_str())])
                        .await?,
                ));
            }
            if !bus.name_has_owner(peer.clone().into()).await?
                || !bus.name_has_owner(client.clone().into()).await?
            {
                return Err(fdo::Error::AccessDenied("Print caller disappeared".into()));
            }
            let (revoked, _) = watch::channel(false);
            let lost = revoked.clone();
            let (synchronize, mut synchronization) = mpsc::channel::<oneshot::Sender<()>>(1);
            let monitor = tokio::spawn(async move {
                let loss = futures_util::future::select_all(monitors);
                tokio::pin!(loss);
                loop {
                    tokio::select! {
                        biased;
                        _ = &mut loss => { lost.send_replace(true); break; }
                        reply = synchronization.recv() => match reply {
                            Some(reply) => { let _ = reply.send(()); }
                            None => break,
                        },
                    }
                }
            });
            let owner = Self(Arc::new(Inner {
                identity: Identity {
                    bus: connection.server_guid().as_str().to_owned(),
                    peer,
                    client,
                    app_id: app_id.to_owned(),
                    frontend,
                },
                connection: connection.clone(),
                revoked,
                synchronize,
                monitor,
            }));
            if !owner.confirm().await {
                return Err(fdo::Error::AccessDenied("Print caller changed".into()));
            }
            Ok(owner)
        })
        .await
        .map_err(|_| fdo::Error::TimedOut("Print caller verification timed out".into()))?
    }

    pub fn client(&self) -> &str {
        self.0.identity.client.as_str()
    }

    pub fn cancelled(&self) -> watch::Receiver<bool> {
        self.0.revoked.subscribe()
    }

    pub(crate) fn is_revoked(&self) -> bool {
        *self.0.revoked.borrow()
    }

    pub(crate) fn same_identity(&self, other: &Self) -> bool {
        self.0.identity == other.0.identity
    }

    pub(crate) fn same_actor(&self, other: &Self) -> bool {
        let first = &self.0.identity;
        let second = &other.0.identity;
        first.bus == second.bus
            && first.peer == second.peer
            && first.client == second.client
            && first.frontend == second.frontend
    }

    /// Check current ownership and process already-queued loss before granting
    /// authority. Query failures/timeouts permanently revoke this proof.
    pub async fn confirm(&self) -> bool {
        if self.is_revoked() {
            return false;
        }
        let confirmed = tokio::time::timeout(BUS_TIMEOUT, async {
            let bus = fdo::DBusProxy::new(&self.0.connection).await?;
            let identity = &self.0.identity;
            if !bus.name_has_owner(identity.peer.clone().into()).await?
                || !bus.name_has_owner(identity.client.clone().into()).await?
                || identity.frontend
                    && bus
                        .get_name_owner(BusName::try_from(FRONTEND).expect("static frontend name"))
                        .await?
                        != identity.peer
            {
                return Ok::<_, fdo::Error>(false);
            }
            let (reply, acknowledged) = oneshot::channel();
            if self.0.synchronize.send(reply).await.is_err() || acknowledged.await.is_err() {
                return Ok(false);
            }
            Ok(!self.is_revoked())
        })
        .await;
        if matches!(confirmed, Ok(Ok(true))) {
            true
        } else {
            self.0.revoked.send_replace(true);
            false
        }
    }
}

#[cfg(test)]
mod tests;
