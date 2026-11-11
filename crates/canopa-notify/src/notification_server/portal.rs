//! Portal transport identity and owned operation admission. Notify retains policy,
//! lifecycle, presentation and input-bound activation ownership.
use canopa_notify::{
    Daemon, DeliveryOutcome, InternalNotificationId, NotificationInput, SenderEvent,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedValue, Value};

mod expiry;
mod intake;
#[cfg(test)]
mod tests;

pub(super) const PATH: &str = "/org/freedesktop/portal/desktop";
pub(super) const BUS_NAME: &str = "org.freedesktop.impl.portal.desktop.canopa.notify";
const REGISTRATION_BYTES: usize = 16 * 1024 * 1024;
pub(super) const EXPIRY_OPTION: &str = "org.canopa.request-timeout-ms";

#[derive(Debug)]
struct RegistrationLimit;

impl std::fmt::Display for RegistrationLimit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("portal notification registration resource limit exceeded")
    }
}

impl std::error::Error for RegistrationLimit {}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Identity {
    app_id: String,
    id: String,
}

struct Action {
    name: String,
    target: Option<OwnedValue>,
}

struct Binding {
    identity: Identity,
    actions: HashMap<String, Action>,
    bytes: usize,
    deadline: Option<Instant>,
}

#[derive(Clone, Default)]
pub(super) struct Registry(Arc<Mutex<State>>, Arc<tokio::sync::Notify>);

#[derive(Default)]
struct State {
    by_identity: HashMap<Identity, InternalNotificationId>,
    by_internal: HashMap<InternalNotificationId, Arc<Binding>>,
    tokens: HashMap<InternalNotificationId, String>,
    replies: HashMap<InternalNotificationId, String>,
    bytes: usize,
    expires: HashMap<InternalNotificationId, (Instant, Weak<Binding>)>,
}

impl Registry {
    fn replacement(&self, identity: &Identity) -> Option<InternalNotificationId> {
        self.0.lock().unwrap().by_identity.get(identity).copied()
    }

    fn admit(&self, binding: &Binding) -> Result<(), RegistrationLimit> {
        let state = self.0.lock().unwrap();
        let previous = state
            .by_identity
            .get(&binding.identity)
            .and_then(|id| state.by_internal.get(id));
        if (previous.is_none() && state.by_internal.len() >= 1024)
            || state.bytes - previous.map_or(0, |item| item.bytes) + binding.bytes
                > REGISTRATION_BYTES
        {
            return Err(RegistrationLimit);
        }
        Ok(())
    }

    fn commit(&self, binding: Arc<Binding>, id: InternalNotificationId) {
        let mut state = self.0.lock().unwrap();
        if let Some(previous) = state.by_internal.insert(id, binding.clone()) {
            state.bytes -= previous.bytes;
            state.by_identity.remove(&previous.identity);
        }
        state.bytes += binding.bytes;
        state.by_identity.insert(binding.identity.clone(), id);
        state.tokens.remove(&id);
        state.replies.remove(&id);
        if let Some(deadline) = binding.deadline {
            state
                .expires
                .insert(id, (deadline, Arc::downgrade(&binding)));
        } else {
            state.expires.remove(&id);
        }
        drop(state);
        self.1.notify_one();
    }

    /// Called synchronously under Notify's mutation coordinator. `Some(None)`
    /// consumes a portal-only event; an unknown registration falls through to FDO.
    pub(super) fn freeze(&self, event: &SenderEvent) -> Option<Option<Publication>> {
        let mut state = self.0.lock().unwrap();
        let id = match event {
            SenderEvent::ActivationToken {
                notification_id, ..
            }
            | SenderEvent::ActionInvoked {
                notification_id, ..
            }
            | SenderEvent::ReplyText {
                notification_id, ..
            }
            | SenderEvent::NotificationClosed {
                notification_id, ..
            } => *notification_id,
        };
        let binding = state.by_internal.get(&id)?.clone();
        Some(match event {
            SenderEvent::ActivationToken {
                activation_token, ..
            } => {
                state.tokens.insert(id, activation_token.clone());
                None
            }
            SenderEvent::ReplyText { text, .. } => {
                state.replies.insert(id, text.clone());
                None
            }
            SenderEvent::ActionInvoked { action_key, .. } => {
                let token = state.tokens.remove(&id);
                // A reply belongs to exactly the next action of its registration.
                let reply = state.replies.remove(&id);
                binding
                    .actions
                    .contains_key(action_key)
                    .then(|| Publication {
                        binding,
                        key: action_key.clone(),
                        token,
                        reply,
                    })
            }
            SenderEvent::NotificationClosed { .. } => {
                state.by_internal.remove(&id);
                state.by_identity.remove(&binding.identity);
                state.tokens.remove(&id);
                state.replies.remove(&id);
                state.expires.remove(&id);
                self.1.notify_one();
                state.bytes -= binding.bytes;
                None
            }
        })
    }
}

pub(super) struct Publication {
    binding: Arc<Binding>,
    key: String,
    token: Option<String>,
    reply: Option<String>,
}

impl Publication {
    pub(super) async fn emit(self, connection: &zbus::Connection) -> zbus::Result<()> {
        let identity = &self.binding.identity;
        let action = &self.binding.actions[&self.key];
        let mut platform = HashMap::<&str, Value<'_>>::new();
        if let Some(token) = &self.token {
            platform.insert("activation-token", Value::from(token.as_str()));
            platform.insert("desktop-startup-id", Value::from(token.as_str()));
        }
        let mut parameters = Vec::new();
        if let Some(target) = &action.target {
            parameters.push((**target).try_clone()?);
        }
        // Portal v2: the response follows the target for exported actions, and
        // the target and platform data for the ActionInvoked signal.
        let reply = self.reply.as_deref().map(Value::from);
        if action.name.is_empty() || action.name.starts_with("app.") {
            parameters.extend(reply);
            let path = format!("/{}", identity.app_id.replace('.', "/").replace('-', "_"));
            // Application failures are per-invocation and must not poison the
            // authoritative sender queue or attach this token to a later click.
            let activation = async {
                if let Some(name) = action.name.strip_prefix("app.") {
                    connection
                        .call_method(
                            Some(identity.app_id.as_str()),
                            path.as_str(),
                            Some("org.freedesktop.Application"),
                            "ActivateAction",
                            &(name, parameters, platform),
                        )
                        .await
                } else {
                    connection
                        .call_method(
                            Some(identity.app_id.as_str()),
                            path.as_str(),
                            Some("org.freedesktop.Application"),
                            "Activate",
                            &(platform,),
                        )
                        .await
                }
            };
            match tokio::time::timeout(std::time::Duration::from_secs(2), activation).await {
                Ok(Ok(_)) => {}
                _ if connection.is_closed() => {
                    return Err(zbus::Error::Failure("notification bus disconnected".into()));
                }
                _ => eprintln!(
                    "level=warn component=portal_notification event=application_activation_failed"
                ),
            }
        } else {
            parameters.push(Value::from(platform));
            parameters.extend(reply);
            let emitter = SignalEmitter::new(connection, PATH)?;
            PortalServer::action_invoked(
                &emitter,
                &identity.app_id,
                &identity.id,
                &action.name,
                parameters,
            )
            .await?;
        }
        Ok(())
    }
}

pub(super) struct PortalServer {
    operations: Operations,
}

type OperationState = Mutex<Option<tokio::sync::mpsc::Sender<Operation>>>;
type Operations = Arc<OperationState>;

pub(super) struct Handle {
    operations: Operations,
    expiry_stop: tokio::sync::watch::Sender<bool>,
    expiry_task: tokio::task::JoinHandle<()>,
}

impl Handle {
    /// Stop new method admission, then drain operations already accepted by the
    /// transport before Notify closes its mutation coordinator.
    pub(super) async fn shutdown(self) -> anyhow::Result<()> {
        let operations = self.operations.lock().unwrap().take();
        self.expiry_stop.send_replace(true);
        self.expiry_task.await?;
        if let Some(operations) = operations {
            let (reply, result) = tokio::sync::oneshot::channel();
            operations
                .send(Operation::CloseQuestions)
                .await
                .map_err(|_| anyhow::anyhow!("portal question drain failed"))?;
            operations
                .send(Operation::Shutdown(reply))
                .await
                .map_err(|_| {
                    anyhow::anyhow!("portal notification operations stopped before drain")
                })?;
            result
                .await
                .map_err(|_| anyhow::anyhow!("portal notification drain failed"))?;
        }
        Ok(())
    }
}

enum Operation {
    Add(
        Identity,
        HashMap<String, OwnedValue>,
        tokio::sync::oneshot::Sender<zbus::fdo::Result<()>>,
        Instant,
    ),
    Remove(
        Identity,
        tokio::sync::oneshot::Sender<zbus::fdo::Result<()>>,
    ),
    Shutdown(tokio::sync::oneshot::Sender<()>),
    Expire(InternalNotificationId, Weak<Binding>),
    CloseQuestions,
}

impl PortalServer {
    pub(super) fn start(daemon: Daemon, registry: Registry) -> (Self, Handle) {
        let (operations, mut received) = tokio::sync::mpsc::channel(32);
        let expiry_registry = registry.clone();
        tokio::spawn(async move {
            while let Some(operation) = received.recv().await {
                match operation {
                    Operation::Shutdown(reply) => {
                        let _ = reply.send(());
                        break;
                    }
                    Operation::Add(identity, values, reply, admitted) => {
                        let prepared = tokio::task::spawn_blocking(move || {
                            intake::prepare_at(identity, values, admitted)
                        })
                        .await;
                        let result = match prepared {
                            Ok(Ok((mut input, binding))) => {
                                if binding
                                    .deadline
                                    .is_some_and(|deadline| deadline <= Instant::now())
                                {
                                    let _ = reply.send(Ok(()));
                                    continue;
                                }
                                let binding = Arc::new(binding);
                                let registered = binding.clone();
                                let prepare_registry = registry.clone();
                                let commit_registry = registry.clone();
                                daemon
                                    .receive_notification_with_preparation(
                                        move || {
                                            prepare_registry.admit(&binding)?;
                                            input.replaces_id =
                                                prepare_registry.replacement(&binding.identity);
                                            Ok(input)
                                        },
                                        move |receipt| {
                                            if receipt.delivery_outcome != DeliveryOutcome::Dropped
                                            {
                                                commit_registry
                                                    .commit(registered, receipt.notification_id);
                                            }
                                        },
                                    )
                                    .await
                                    .map(|_| ())
                                    .map_err(receipt_error)
                            }
                            Ok(Err(error)) => Err(error),
                            Err(_) => Err(zbus::fdo::Error::Failed(
                                "notification preparation failed".into(),
                            )),
                        };
                        let _ = reply.send(result);
                    }
                    Operation::Remove(identity, reply) => {
                        let result = if let Some(id) = registry.replacement(&identity) {
                            daemon.close_notification(id).await.map_err(receipt_error)
                        } else {
                            Ok(())
                        };
                        let _ = reply.send(result);
                    }
                    Operation::Expire(id, binding) => {
                        let matches = binding.upgrade().is_some_and(|expected| {
                            registry
                                .0
                                .lock()
                                .unwrap()
                                .by_internal
                                .get(&id)
                                .is_some_and(|current| Arc::ptr_eq(current, &expected))
                        });
                        if matches && daemon.close_notification(id).await.is_err() {
                            eprintln!(
                                "level=warn component=portal_notification event=request_expiry_failed"
                            );
                        }
                    }
                    Operation::CloseQuestions => {
                        let ids: Vec<_> = registry
                            .0
                            .lock()
                            .unwrap()
                            .by_internal
                            .iter()
                            .filter(|(_, binding)| binding.deadline.is_some())
                            .map(|(id, _)| *id)
                            .collect();
                        for id in ids {
                            if daemon.close_notification(id).await.is_err() {
                                eprintln!(
                                    "level=warn component=portal_notification event=request_shutdown_withdrawal_failed"
                                );
                            }
                        }
                    }
                }
            }
        });
        let operations = Arc::new(Mutex::new(Some(operations)));
        let (expiry_stop, stopped) = tokio::sync::watch::channel(false);
        let expiry_task = tokio::spawn(expiry::run(
            expiry_registry,
            Arc::downgrade(&operations),
            stopped,
        ));
        (
            Self {
                operations: operations.clone(),
            },
            Handle {
                operations,
                expiry_stop,
                expiry_task,
            },
        )
    }

    fn admit(&self, operation: Operation) -> zbus::fdo::Result<()> {
        self.operations
            .lock()
            .unwrap()
            .as_ref()
            .ok_or_else(|| {
                zbus::fdo::Error::Failed("notification operation service stopping".into())
            })?
            .try_send(operation)
            .map_err(|_| {
                zbus::fdo::Error::LimitsExceeded("notification operation capacity exhausted".into())
            })
    }

    fn identity(app_id: String, id: String) -> zbus::fdo::Result<Identity> {
        if app_id.len() > 255
            || zbus::names::WellKnownName::try_from(app_id.as_str()).is_err()
            || id.len() > 4096
        {
            return Err(zbus::fdo::Error::InvalidArgs(
                "invalid portal application or notification ID".into(),
            ));
        }
        Ok(Identity { app_id, id })
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Notification")]
impl PortalServer {
    async fn add_notification(
        &self,
        app_id: String,
        id: String,
        notification: HashMap<String, OwnedValue>,
    ) -> zbus::fdo::Result<()> {
        let identity = Self::identity(app_id, id)?;
        if notification.len() > 64 {
            return Err(zbus::fdo::Error::LimitsExceeded(
                "notification field limit exceeded".into(),
            ));
        }
        intake::wire_budget(&notification)?;
        let (reply, result) = tokio::sync::oneshot::channel();
        self.admit(Operation::Add(
            identity,
            notification,
            reply,
            Instant::now(),
        ))?;
        result.await.map_err(|_| {
            zbus::fdo::Error::Failed("notification operation service stopped".into())
        })?
    }

    async fn remove_notification(&self, app_id: String, id: String) -> zbus::fdo::Result<()> {
        let identity = Self::identity(app_id, id)?;
        let (reply, result) = tokio::sync::oneshot::channel();
        self.admit(Operation::Remove(identity, reply))?;
        result.await.map_err(|_| {
            zbus::fdo::Error::Failed("notification operation service stopped".into())
        })?
    }

    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        2
    }

    #[zbus(property)]
    fn supported_options(&self) -> HashMap<String, OwnedValue> {
        let list = |values: &[&str]| {
            OwnedValue::try_from(Value::from(
                values
                    .iter()
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>(),
            ))
            .expect("string arrays have no descriptors")
        };
        HashMap::from([
            (EXPIRY_OPTION.into(), true.into()),
            ("category".into(), list(intake::SUPPORTED_CATEGORIES)),
            ("button-purpose".into(), list(intake::SUPPORTED_PURPOSES)),
        ])
    }

    #[zbus(signal)]
    async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        app_id: &str,
        id: &str,
        action: &str,
        parameter: Vec<Value<'_>>,
    ) -> zbus::Result<()>;
}

fn receipt_error(error: anyhow::Error) -> zbus::fdo::Error {
    if error.is::<canopa_notify::NotificationLimitExceeded>() || error.is::<RegistrationLimit>() {
        zbus::fdo::Error::LimitsExceeded("notification resource limit exceeded".into())
    } else {
        zbus::fdo::Error::Failed("notification operation failed".into())
    }
}
