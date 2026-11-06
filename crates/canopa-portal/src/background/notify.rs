//! Request-owned questions rendered by Canopa Notify. Deterministic notification
//! IDs keep removal possible through a lost Add reply. Cleanup retains its slot.
use super::{BackgroundRequest, Decision, Presenter};
use crate::process::DialogProcess;
use futures_util::{StreamExt, future::BoxFuture};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{Semaphore, oneshot, watch};
use zbus::{
    Connection, Proxy,
    fdo::DBusProxy,
    zvariant::{OwnedValue, Value},
};

const NOTIFY: &str = "org.freedesktop.impl.portal.desktop.canopa.notify";
const PATH: &str = "/org/freedesktop/portal/desktop";
const APP: &str = "org.canopa.Portal";
const EXPIRY_OPTION: &str = "org.canopa.request-timeout-ms";
const SLOTS: u32 = 16;

#[derive(Clone, Copy)]
struct Policy {
    lifetime: Duration,
    rpc: Duration,
    cleanup: Duration,
}
const POLICY: Policy = Policy {
    lifetime: Duration::from_secs(20),
    rpc: Duration::from_secs(2),
    cleanup: Duration::from_secs(5),
};

pub struct NotifyPresenter {
    connection: Connection,
    process: Arc<DialogProcess>,
    slots: Arc<Semaphore>,
    stopped: watch::Sender<bool>,
    policy: Policy,
}
impl NotifyPresenter {
    pub fn new(connection: Connection, process: Arc<DialogProcess>) -> Self {
        let (stopped, _) = watch::channel(false);
        Self {
            connection,
            process,
            slots: Arc::new(Semaphore::new(SLOTS as usize)),
            stopped,
            policy: POLICY,
        }
    }

    pub async fn shutdown(&self) -> anyhow::Result<()> {
        self.stopped.send_replace(true);
        let _all = tokio::time::timeout(
            self.policy.cleanup + self.policy.rpc,
            self.slots.clone().acquire_many_owned(SLOTS),
        )
        .await??;
        Ok(())
    }
}
impl Presenter for NotifyPresenter {
    fn present(
        &self,
        request: BackgroundRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Decision>> {
        let connection = self.connection.clone();
        let process = self.process.clone();
        let slots = self.slots.clone();
        let stopped = self.stopped.subscribe();
        let policy = self.policy;
        Box::pin(async move {
            let slot = slots.try_acquire_owned()?;
            let (reply, result) = oneshot::channel();
            // Requests may drop this future on Close or caller loss. The owned
            // actor still receives cancellation and removes its notification.
            tokio::spawn(async move {
                question(
                    connection, process, request, cancelled, stopped, policy, reply,
                )
                .await;
                drop(slot);
            });
            result.await?
        })
    }
}

async fn cancelled(receiver: &mut watch::Receiver<bool>) {
    if !*receiver.borrow() {
        let _ = receiver.changed().await;
    }
}

fn id() -> anyhow::Result<String> {
    let mut bytes = [0u8; 16];
    let count = rustix::rand::getrandom(&mut bytes[..], rustix::rand::GetRandomFlags::NONBLOCK)?;
    anyhow::ensure!(count == bytes.len(), "question identity unavailable");
    Ok(format!(
        "background-{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ))
}
fn value(value: impl Into<Value<'static>>) -> OwnedValue {
    OwnedValue::try_from(value.into()).expect("owned question value")
}
fn content(request: &BackgroundRequest) -> HashMap<String, OwnedValue> {
    let buttons = [
        ("Block", "canopa.background.forbid", 0u32),
        ("Allow once", "canopa.background.once", 2),
        ("Allow", "canopa.background.allow", 1),
    ]
    .into_iter()
    .map(|(label, action, target)| {
        HashMap::from([
            ("label".to_owned(), value(label)),
            ("action".into(), value(action)),
            ("target".into(), target.into()),
        ])
    })
    .collect::<Vec<_>>();
    HashMap::from([
        (
            "title".into(),
            value(format!(
                "{} is running in the background",
                request.name.chars().take(120).collect::<String>()
            )),
        ),
        (
            "body".into(),
            value(format!(
                "Allow {} to keep running without open windows? Choose Allow once for its current run only.",
                request.app_id
            )),
        ),
        ("priority".into(), value("normal")),
        ("sound".into(), value("silent")),
        ("default-action".into(), value("canopa.background.review")),
        ("default-action-target".into(), 3u32.into()),
        ("buttons".into(), value(Value::from(buttons))),
    ])
}

enum Action {
    Decide(Decision),
    Review(Option<String>),
}
fn action(message: &zbus::Message, identity: &str) -> Option<Action> {
    let (app, id, name, parameters): (String, String, String, Vec<OwnedValue>) =
        message.body().deserialize().ok()?;
    if app != APP || id != identity || parameters.len() != 2 {
        return None;
    }
    let target = u32::try_from(parameters[0].try_clone().ok()?).ok()?;
    let platform = HashMap::<String, OwnedValue>::try_from(parameters[1].try_clone().ok()?).ok()?;
    match (name.as_str(), target) {
        ("canopa.background.forbid", 0) => Some(Action::Decide(Decision::Forbid)),
        ("canopa.background.allow", 1) => Some(Action::Decide(Decision::Allow)),
        ("canopa.background.once", 2) => Some(Action::Decide(Decision::AllowOnce)),
        ("canopa.background.review", 3) => {
            let token = platform
                .get("activation-token")
                .and_then(|v| <&str>::try_from(v).ok())
                .filter(|token| !token.is_empty() && token.len() <= 4096 && !token.contains('\0'))
                .map(str::to_owned);
            Some(Action::Review(token))
        }
        _ => None,
    }
}

async fn question(
    connection: Connection,
    process: Arc<DialogProcess>,
    request: BackgroundRequest,
    mut cancellation: watch::Receiver<bool>,
    mut stopped: watch::Receiver<bool>,
    policy: Policy,
    reply: oneshot::Sender<anyhow::Result<Decision>>,
) {
    let deadline = tokio::time::Instant::now() + policy.lifetime;
    let setup = async {
        let bus = DBusProxy::new(&connection).await?;
        let mut owners = bus
            .receive_name_owner_changed_with_args(&[(0, NOTIFY)])
            .await?;
        let owner = match bus
            .get_name_owner(NOTIFY.try_into().expect("notify name"))
            .await
        {
            Ok(owner) => owner,
            Err(_) => {
                bus.start_service_by_name(NOTIFY.try_into().expect("notify name"), 0)
                    .await?;
                bus.get_name_owner(NOTIFY.try_into().expect("notify name"))
                    .await?
            }
        };
        let proxy = Proxy::new(
            &connection,
            owner,
            PATH,
            "org.freedesktop.impl.portal.Notification",
        )
        .await?;
        let version: u32 = proxy.get_property("version").await?;
        anyhow::ensure!(version >= 2, "notification backend version unavailable");
        let options: HashMap<String, OwnedValue> = proxy.get_property("SupportedOptions").await?;
        let expiry = options
            .get(EXPIRY_OPTION)
            .and_then(|value| value.try_clone().ok())
            .and_then(|value| bool::try_from(value).ok())
            .unwrap_or(false);
        anyhow::ensure!(expiry, "notification request expiry unavailable");
        // Activation itself changes the owner once; drain only changes already
        // observed, then revalidate the exact unique name before sending work.
        while owners.next().now_or_never().flatten().is_some() {}
        anyhow::ensure!(
            bus.get_name_owner(NOTIFY.try_into().expect("notify name"))
                .await?
                .as_str()
                == proxy.destination().as_str(),
            "notification owner changed"
        );
        Ok::<_, anyhow::Error>((proxy, owners))
    };
    use futures_util::FutureExt;
    let configured = tokio::select! {
        biased;
        _ = cancelled(&mut cancellation) => { let _ = reply.send(Ok(Decision::AllowOnce)); return; },
        _ = cancelled(&mut stopped) => { let _ = reply.send(Ok(Decision::AllowOnce)); return; },
        // Cold service activation includes renderer/history startup. It gets a
        // separate bounded allowance inside the overall question lifetime;
        // ordinary Add/Remove RPCs retain their short deadline.
        result = tokio::time::timeout(policy.lifetime.min(Duration::from_secs(10)), setup) => result.map_err(anyhow::Error::from).and_then(|value| value),
    };
    let (proxy, owners) = match configured {
        Ok(value) => value,
        Err(error) => {
            let _ = reply.send(Err(error));
            return;
        }
    };
    let identity = match id() {
        Ok(id) => id,
        Err(error) => {
            let _ = reply.send(Err(error));
            return;
        }
    };
    let subscribed = tokio::time::timeout(
        policy.rpc,
        proxy.receive_signal_with_args("ActionInvoked", &[(0, APP), (1, &identity)]),
    )
    .await;
    let mut actions = match subscribed {
        Ok(Ok(stream)) => stream,
        _ => {
            let _ = reply.send(Err(anyhow::anyhow!("question callback unavailable")));
            return;
        }
    };
    if *cancellation.borrow() || *stopped.borrow() || tokio::time::Instant::now() >= deadline {
        let _ = reply.send(Ok(Decision::AllowOnce));
        return;
    }
    let expected = proxy.destination().as_str().to_owned();
    let mut losses = owners.filter_map(move |signal| {
        let lost = signal.args().ok().is_none_or(|args| {
            args.new_owner().as_ref().map(|name| name.as_str()) != Some(expected.as_str())
        });
        std::future::ready(lost.then_some(()))
    });
    let mut notification = content(&request);
    notification.insert(
        EXPIRY_OPTION.into(),
        (policy.lifetime.as_millis().clamp(1, 30_000) as u32).into(),
    );
    // Complete or time out the Add call even if the request is cancelled. The
    // deterministic ID and ordered Remove also cover accepted work/reply loss.
    let added: Result<zbus::Result<()>, _> = tokio::time::timeout(
        policy.rpc,
        proxy.call("AddNotification", &(APP, &identity, notification)),
    )
    .await;
    let mut owner_lost = false;
    let result = match added {
        Ok(Ok(())) => loop {
            let next = tokio::select! {
                biased;
                _ = cancelled(&mut cancellation) => break Ok(Decision::AllowOnce),
                _ = cancelled(&mut stopped) => break Ok(Decision::AllowOnce),
                _ = tokio::time::sleep_until(deadline) => break Ok(Decision::AllowOnce),
                _ = losses.next() => { owner_lost = true; break Ok(Decision::AllowOnce); },
                next = actions.next() => next,
            };
            let Some(message) = next else {
                owner_lost = true;
                break Ok(Decision::AllowOnce);
            };
            match action(&message, &identity) {
                Some(Action::Decide(decision)) => break Ok(decision),
                Some(Action::Review(token)) => {
                    let dialog = process.exchange_activated::<_, Decision>(
                        "--background-dialog",
                        &request,
                        cancellation.clone(),
                        token.as_deref(),
                    );
                    break tokio::select! {
                        biased;
                        _ = cancelled(&mut cancellation) => Ok(Decision::AllowOnce),
                        _ = cancelled(&mut stopped) => Ok(Decision::AllowOnce),
                        _ = tokio::time::sleep_until(deadline) => Ok(Decision::AllowOnce),
                        _ = losses.next() => { owner_lost = true; Ok(Decision::AllowOnce) },
                        result = dialog => result.map(|decision| decision.unwrap_or(Decision::AllowOnce)),
                    };
                }
                None => {}
            }
        },
        Ok(Err(error)) => Err(error.into()),
        Err(error) => Err(error.into()),
    };
    let _ = reply.send(result);
    // Cleanup owns its actor/slot after the method's response. Retry temporary
    // admission/transport failures without creating an unbounded task backlog.
    let cleanup_deadline = tokio::time::Instant::now() + policy.cleanup;
    loop {
        let removed: Result<zbus::Result<()>, _> = tokio::time::timeout(
            policy.rpc,
            proxy.call("RemoveNotification", &(APP, &identity)),
        )
        .await;
        if matches!(removed, Ok(Ok(()))) || connection.is_closed() || owner_lost {
            break;
        }
        if tokio::time::Instant::now() >= cleanup_deadline {
            eprintln!("level=warn component=background_portal event=question_cleanup_unconfirmed");
            break;
        }
        tokio::select! {
            _ = losses.next() => break,
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
    }
}

#[cfg(test)]
mod tests;
