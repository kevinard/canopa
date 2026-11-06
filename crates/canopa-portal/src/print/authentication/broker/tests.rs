use super::*;
use crate::print::authentication::{Endpoint, Field, Secret, Transport};
use std::sync::atomic::{AtomicUsize, Ordering};

fn challenge() -> Challenge {
    Challenge {
        version: 1,
        interactive: true,
        id: 1,
        endpoint: Endpoint {
            server: "office.example".into(),
            port: 631,
            transport: Transport::Tls,
        },
        kind: Kind::Server {
            limits: crate::print::authentication::ServerLimits::Legacy,
            realm: "Office".into(),
            resource: "/".into(),
        },
        attempt: 1,
        suggested_user: "initial".into(),
    }
}
fn answer(challenge: &Challenge) -> Credentials {
    Credentials {
        values: challenge
            .fields()
            .iter()
            .map(|field| {
                Secret::copy(match field {
                    Field::Domain => b"DOMAIN",
                    Field::Username => b"chosen-user",
                    Field::Password => b"synthetic,private\\password",
                })
            })
            .collect(),
    }
}
#[derive(Default)]
struct Prompts {
    calls: AtomicUsize,
    shown: Mutex<Vec<Challenge>>,
}
impl Presenter for Prompts {
    fn show(
        &self,
        challenge: Challenge,
        _: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<Credentials>>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.shown.lock().unwrap().push(challenge.clone());
        Box::pin(async move { Ok(Some(answer(&challenge))) })
    }
}
fn broker() -> (Arc<Broker>, Arc<Prompts>, watch::Sender<bool>) {
    let prompts = Arc::new(Prompts::default());
    let broker = Broker::with_presenter(Retained::default(), prompts.clone());
    let (cancel, _) = watch::channel(false);
    (broker, prompts, cancel)
}
#[tokio::test]
async fn cached_exchanges_do_not_consume_user_prompt_budget() {
    let (broker, prompts, cancel) = broker();
    for _ in 0..64 {
        assert!(
            broker
                .request(challenge(), cancel.subscribe())
                .await
                .unwrap()
                .is_some()
        );
    }
    assert_eq!(prompts.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cache_only_requests_reuse_exact_approved_values_without_new_prompts() {
    let (broker, prompts, cancel) = broker();
    let mut cached = challenge();
    cached.interactive = false;
    cached.suggested_user = "chosen-user".into();
    assert!(
        broker
            .request(cached.clone(), cancel.subscribe())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(prompts.calls.load(Ordering::SeqCst), 0);
    broker
        .request(challenge(), cancel.subscribe())
        .await
        .unwrap()
        .unwrap();
    assert!(
        broker
            .request(cached.clone(), cancel.subscribe())
            .await
            .unwrap()
            .is_some()
    );
    let mut changed = cached.clone();
    changed.endpoint.port += 1;
    assert!(
        broker
            .request(changed, cancel.subscribe())
            .await
            .unwrap()
            .is_none()
    );
    changed = cached.clone();
    changed.kind = Kind::Server {
        limits: crate::print::authentication::ServerLimits::Legacy,
        realm: "Other".into(),
        resource: "/".into(),
    };
    assert!(
        broker
            .request(changed, cancel.subscribe())
            .await
            .unwrap()
            .is_none()
    );
    cached.kind = Kind::Queue {
        destination: Destination {
            queue: "Other".into(),
            instance: None,
        },
        fields: vec![Field::Password],
    };
    assert!(
        broker
            .request(cached, cancel.subscribe())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(prompts.calls.load(Ordering::SeqCst), 1);
    assert_eq!(broker.state.lock().unwrap().retained.prompts, 1);
}

#[tokio::test]
async fn automatic_retirement_cannot_switch_to_a_different_approved_server_account() {
    let (broker, prompts, cancel) = broker();
    broker
        .request(challenge(), cancel.subscribe())
        .await
        .unwrap()
        .unwrap();
    let mut cached = challenge();
    cached.interactive = false;
    assert!(
        broker
            .request(cached.clone(), cancel.subscribe())
            .await
            .unwrap()
            .is_none()
    );
    cached.suggested_user = "chosen-user".into();
    assert!(
        broker
            .request(cached, cancel.subscribe())
            .await
            .unwrap()
            .is_some()
    );
    // Keep the approved account available for a subsequent explicit operation.
    let values = broker
        .request(challenge(), cancel.subscribe())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(values.values[0].bytes(), b"chosen-user");
    assert_eq!(prompts.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn rejected_cached_credentials_cannot_open_a_retry_dialog_or_consume_prompt_budget() {
    let (broker, prompts, cancel) = broker();
    broker
        .request(challenge(), cancel.subscribe())
        .await
        .unwrap()
        .unwrap();
    let mut rejected = challenge();
    rejected.interactive = false;
    rejected.suggested_user = "chosen-user".into();
    rejected.attempt = 2;
    assert!(
        broker
            .request(rejected, cancel.subscribe())
            .await
            .unwrap()
            .is_none()
    );
    let mut cached = challenge();
    cached.interactive = false;
    assert!(
        broker
            .request(cached, cancel.subscribe())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(prompts.calls.load(Ordering::SeqCst), 1);
    assert_eq!(broker.state.lock().unwrap().retained.prompts, 1);
    assert!(broker.state.lock().unwrap().retained.entries.is_empty());
}
#[tokio::test]
async fn endpoint_protection_realm_and_resource_changes_require_new_consent() {
    let (broker, prompts, cancel) = broker();
    let base = challenge();
    broker
        .request(base.clone(), cancel.subscribe())
        .await
        .unwrap();
    let mut variants = vec![base.clone(); 5];
    variants[0].endpoint.server = "other.example".into();
    variants[1].endpoint.port = 632;
    variants[2].endpoint.transport = Transport::Loopback;
    variants[3].kind = Kind::Server {
        limits: crate::print::authentication::ServerLimits::Legacy,
        realm: "Other".into(),
        resource: "/".into(),
    };
    variants[4].kind = Kind::Server {
        limits: crate::print::authentication::ServerLimits::Legacy,
        realm: "Office".into(),
        resource: "/printers/Other".into(),
    };
    for changed in variants {
        broker.request(changed, cancel.subscribe()).await.unwrap();
    }
    assert_eq!(prompts.calls.load(Ordering::SeqCst), 6);
}
#[tokio::test]
async fn queue_and_ordered_fields_are_separate_scopes_and_only_selected_queue_is_retained() {
    let (broker, prompts, cancel) = broker();
    let destination = Destination {
        queue: "Office".into(),
        instance: None,
    };
    let mut c = challenge();
    c.kind = Kind::Queue {
        destination: destination.clone(),
        fields: vec![Field::Domain, Field::Username, Field::Password],
    };
    broker.request(c.clone(), cancel.subscribe()).await.unwrap();
    c.kind = Kind::Queue {
        destination: destination.clone(),
        fields: vec![Field::Password, Field::Username, Field::Domain],
    };
    broker.request(c.clone(), cancel.subscribe()).await.unwrap();
    let other = Destination {
        queue: "Other".into(),
        instance: None,
    };
    c.kind = Kind::Queue {
        destination: other,
        fields: vec![Field::Password],
    };
    broker.request(c, cancel.subscribe()).await.unwrap();
    assert_eq!(prompts.calls.load(Ordering::SeqCst), 3);
    let retained = broker.take(&destination);
    assert_eq!(retained.entries.len(), 2);
    assert!(broker.state.lock().unwrap().retained.entries.is_empty());
}
#[tokio::test]
async fn rejected_credentials_are_invalidated_username_preserved_and_prompt_budget_survives_clear()
{
    let (broker, prompts, cancel) = broker();
    let mut c = challenge();
    broker.request(c.clone(), cancel.subscribe()).await.unwrap();
    c.attempt = 2;
    for _ in 0..5 {
        broker.request(c.clone(), cancel.subscribe()).await.unwrap();
    }
    assert_eq!(
        prompts.shown.lock().unwrap()[1].suggested_user,
        "chosen-user"
    );
    assert!(broker.request(c.clone(), cancel.subscribe()).await.is_err());
    broker.clear();
    c.attempt = 1;
    assert!(broker.request(c, cancel.subscribe()).await.is_err());
    assert_eq!(prompts.calls.load(Ordering::SeqCst), 6);
}
struct Paused {
    started: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
impl Presenter for Paused {
    fn show(
        &self,
        challenge: Challenge,
        _: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<Credentials>>> {
        Box::pin(async move {
            self.started.notify_one();
            self.release.notified().await;
            Ok(Some(answer(&challenge)))
        })
    }
}
#[tokio::test]
async fn file_selection_or_caller_loss_cannot_retain_a_late_prompt_answer() {
    for cancel_caller in [false, true] {
        let presenter = Arc::new(Paused {
            started: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        });
        let broker = Broker::with_presenter(Retained::default(), presenter.clone());
        let (cancel, cancelled) = watch::channel(false);
        let b = broker.clone();
        let pending = tokio::spawn(async move { b.request(challenge(), cancelled).await });
        presenter.started.notified().await;
        if cancel_caller {
            drop(cancel);
        } else {
            broker.clear();
        }
        presenter.release.notify_one();
        assert!(pending.await.unwrap().unwrap().is_none());
        assert!(broker.state.lock().unwrap().retained.entries.is_empty());
    }
}
#[tokio::test]
async fn retained_cache_reuses_credentials_without_resetting_prompt_count() {
    let (broker, prompts, cancel) = broker();
    broker
        .request(challenge(), cancel.subscribe())
        .await
        .unwrap();
    let retained = broker.take(&Destination {
        queue: "Office".into(),
        instance: None,
    });
    let restored = Broker::with_presenter(retained, prompts.clone());
    restored
        .request(challenge(), cancel.subscribe())
        .await
        .unwrap();
    assert_eq!(prompts.calls.load(Ordering::SeqCst), 1);
    assert_eq!(restored.state.lock().unwrap().retained.prompts, 1);
}

#[tokio::test]
async fn changing_runtime_capacity_never_reuses_a_cached_approval() {
    let (broker, prompts, cancel) = broker();
    broker
        .request(challenge(), cancel.subscribe())
        .await
        .unwrap()
        .unwrap();
    let mut changed = challenge();
    let Kind::Server { limits, .. } = &mut changed.kind else {
        unreachable!()
    };
    *limits = super::super::ServerLimits::CanopaV1;
    changed.interactive = false;
    changed.suggested_user = "chosen-user".into();
    assert!(
        broker
            .request(changed.clone(), cancel.subscribe())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(prompts.calls.load(Ordering::SeqCst), 1);
    changed.interactive = true;
    broker
        .request(changed, cancel.subscribe())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(prompts.calls.load(Ordering::SeqCst), 2);
}
