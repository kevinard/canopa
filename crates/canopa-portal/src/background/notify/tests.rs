use super::*;

fn signal(app: &str, identity: &str, action: &str, parameters: Vec<OwnedValue>) -> zbus::Message {
    zbus::Message::signal(
        PATH,
        "org.freedesktop.impl.portal.Notification",
        "ActionInvoked",
    )
    .unwrap()
    .build(&(app, identity, action, parameters))
    .unwrap()
}
fn parameters(target: u32) -> Vec<OwnedValue> {
    vec![target.into(), value(HashMap::<String, OwnedValue>::new())]
}

#[test]
fn only_bound_typed_actions_produce_decisions_or_review() {
    for (name, target, expected) in [
        ("forbid", 0, Decision::Forbid),
        ("allow", 1, Decision::Allow),
        ("once", 2, Decision::AllowOnce),
    ] {
        let message = signal(
            APP,
            "question",
            &format!("canopa.background.{name}"),
            parameters(target),
        );
        assert!(
            matches!(action(&message, "question"), Some(Action::Decide(decision)) if decision == expected)
        );
    }
    for message in [
        signal(
            "org.canopa.Other",
            "question",
            "canopa.background.allow",
            parameters(1),
        ),
        signal(APP, "other", "canopa.background.allow", parameters(1)),
        signal(APP, "question", "canopa.background.allow", parameters(0)),
        signal(
            APP,
            "question",
            "canopa.background.allow",
            vec![1u32.into()],
        ),
        signal(
            APP,
            "question",
            "canopa.background.allow",
            vec![1u32.into(), value("bad platform")],
        ),
        signal(APP, "question", "unknown", parameters(1)),
    ] {
        assert!(action(&message, "question").is_none());
    }
    let platform = value(HashMap::from([(
        "activation-token".to_owned(),
        value("fresh-fixture-token"),
    )]));
    let message = signal(
        APP,
        "question",
        "canopa.background.review",
        vec![3u32.into(), platform],
    );
    assert!(
        matches!(action(&message, "question"), Some(Action::Review(Some(token))) if token == "fresh-fixture-token")
    );
}

#[test]
fn question_content_is_plain_and_keeps_all_three_decisions_distinct() {
    let request =
        BackgroundRequest::parse("org.canopa.Fixture".into(), "Literal <b>name</b>".into())
            .unwrap();
    let fields = content(&request);
    assert!(!fields.contains_key("markup-body"));
    assert!(
        <&str>::try_from(&fields["title"])
            .unwrap()
            .contains("<b>name</b>")
    );
    assert_eq!(
        <&str>::try_from(&fields["default-action"]).unwrap(),
        "canopa.background.review"
    );
    let buttons =
        Vec::<HashMap<String, OwnedValue>>::try_from(fields["buttons"].try_clone().unwrap())
            .unwrap();
    assert_eq!(buttons.len(), 3);
    let targets: Vec<_> = buttons
        .iter()
        .map(|button| u32::try_from(button["target"].try_clone().unwrap()).unwrap())
        .collect();
    assert_eq!(targets, [0, 2, 1]);
    let first = id().unwrap();
    assert!(first.starts_with("background-"));
    assert_ne!(first, id().unwrap());
}

use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
};
use tokio::sync::{Notify, mpsc};

struct Bus(Child);
impl Drop for Bus {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}
struct Added {
    app: String,
    id: String,
}
struct Fake {
    added: mpsc::UnboundedSender<Added>,
    removed: mpsc::UnboundedSender<Added>,
    hold_add: bool,
    hold_remove: Option<Arc<Notify>>,
    metadata_delay: Duration,
}
#[zbus::interface(name = "org.freedesktop.impl.portal.Notification")]
impl Fake {
    async fn add_notification(
        &self,
        app_id: String,
        id: String,
        notification: HashMap<String, OwnedValue>,
    ) {
        assert_eq!(
            Vec::<HashMap<String, OwnedValue>>::try_from(
                notification["buttons"].try_clone().unwrap()
            )
            .unwrap()
            .len(),
            3
        );
        self.added.send(Added { app: app_id, id }).unwrap();
        if self.hold_add {
            std::future::pending::<()>().await;
        }
    }
    async fn remove_notification(&self, app_id: String, id: String) {
        self.removed.send(Added { app: app_id, id }).unwrap();
        if let Some(gate) = &self.hold_remove {
            gate.notified().await;
        }
    }
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        2
    }
    #[zbus(property)]
    async fn supported_options(&self) -> HashMap<String, OwnedValue> {
        tokio::time::sleep(self.metadata_delay).await;
        HashMap::from([(EXPIRY_OPTION.into(), true.into())])
    }
}
struct Fixture {
    _bus: Bus,
    server: Connection,
    client: Connection,
    added: mpsc::UnboundedReceiver<Added>,
    removed: mpsc::UnboundedReceiver<Added>,
}
impl Fixture {
    async fn new(hold_add: bool, hold_remove: Option<Arc<Notify>>) -> Self {
        Self::delayed(hold_add, hold_remove, Duration::ZERO).await
    }
    async fn delayed(
        hold_add: bool,
        hold_remove: Option<Arc<Notify>>,
        metadata_delay: Duration,
    ) -> Self {
        let mut bus = Bus(Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap());
        let mut address = String::new();
        BufReader::new(bus.0.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let (added, adds) = mpsc::unbounded_channel();
        let (removed, removes) = mpsc::unbounded_channel();
        let server = zbus::connection::Builder::address(address.trim())
            .unwrap()
            .serve_at(
                PATH,
                Fake {
                    added,
                    removed,
                    hold_add,
                    hold_remove,
                    metadata_delay,
                },
            )
            .unwrap()
            .name(NOTIFY)
            .unwrap()
            .build()
            .await
            .unwrap();
        let client = zbus::connection::Builder::address(address.trim())
            .unwrap()
            .build()
            .await
            .unwrap();
        Self {
            _bus: bus,
            server,
            client,
            added: adds,
            removed: removes,
        }
    }
    fn presenter(&self, process: DialogProcess) -> Arc<NotifyPresenter> {
        let mut presenter = NotifyPresenter::new(self.client.clone(), Arc::new(process));
        presenter.policy = Policy {
            lifetime: Duration::from_secs(3),
            rpc: Duration::from_millis(300),
            cleanup: Duration::from_secs(1),
        };
        Arc::new(presenter)
    }
    async fn emit(&self, id: &str, name: &str, parameters: Vec<OwnedValue>) {
        self.server
            .emit_signal(
                None::<&str>,
                PATH,
                "org.freedesktop.impl.portal.Notification",
                "ActionInvoked",
                &(APP, id, name, parameters),
            )
            .await
            .unwrap();
    }
}
async fn receive(queue: &mut mpsc::UnboundedReceiver<Added>) -> Added {
    tokio::time::timeout(Duration::from_secs(3), queue.recv())
        .await
        .unwrap()
        .unwrap()
}
fn process() -> DialogProcess {
    DialogProcess {
        executable: "/usr/bin/false".into(),
        arguments: Vec::new(),
    }
}
fn request() -> BackgroundRequest {
    BackgroundRequest::parse("org.canopa.Fixture".into(), "Fixture".into()).unwrap()
}

#[tokio::test]
async fn service_setup_can_outlast_an_ordinary_rpc_without_losing_the_question() {
    let mut fixture = Fixture::delayed(false, None, Duration::from_millis(500)).await;
    let presenter = fixture.presenter(process());
    let (_cancel, cancelled) = watch::channel(false);
    let pending = tokio::spawn({
        let presenter = presenter.clone();
        async move { presenter.present(request(), cancelled).await }
    });
    let added = receive(&mut fixture.added).await;
    fixture
        .emit(&added.id, "canopa.background.once", parameters(2))
        .await;
    assert_eq!(pending.await.unwrap().unwrap(), Decision::AllowOnce);
    assert_eq!(receive(&mut fixture.removed).await.id, added.id);
    presenter.shutdown().await.unwrap();
}

#[tokio::test]
async fn request_retirement_keeps_cleanup_owned_until_remove_finishes() {
    let gate = Arc::new(Notify::new());
    let mut fixture = Fixture::new(false, Some(gate.clone())).await;
    let presenter = fixture.presenter(process());
    let (cancel, cancelled) = watch::channel(false);
    let pending = tokio::spawn({
        let presenter = presenter.clone();
        async move { presenter.present(request(), cancelled).await }
    });
    let added = receive(&mut fixture.added).await;
    assert_eq!(added.app, APP);
    cancel.send_replace(true);
    pending.abort();
    let removed = receive(&mut fixture.removed).await;
    assert_eq!(removed.id, added.id);
    assert_eq!(presenter.slots.available_permits(), SLOTS as usize - 1);
    gate.notify_one();
    presenter.shutdown().await.unwrap();
    assert_eq!(presenter.slots.available_permits(), SLOTS as usize);
}

#[tokio::test]
async fn a_lost_add_reply_still_removes_the_exact_accepted_notification() {
    let mut fixture = Fixture::new(true, None).await;
    let presenter = fixture.presenter(process());
    let (_cancel, cancelled) = watch::channel(false);
    let pending = tokio::spawn({
        let presenter = presenter.clone();
        async move { presenter.present(request(), cancelled).await }
    });
    let added = receive(&mut fixture.added).await;
    assert!(pending.await.unwrap().is_err());
    let removed = receive(&mut fixture.removed).await;
    assert_eq!(removed.id, added.id);
    presenter.shutdown().await.unwrap();
}

#[tokio::test]
async fn only_the_bound_notification_owner_can_answer_and_review_passes_its_token() {
    let mut fixture = Fixture::new(false, None).await;
    let temporary = tempfile::tempdir().unwrap();
    let script = temporary.path().join("helper.sh");
    let token = temporary.path().join("token");
    std::fs::write(
        &script,
        "cat >/dev/null\nprintf '%s' \"${XDG_ACTIVATION_TOKEN-}\" > \"$1\"\nprintf '\"Allow\"'\n",
    )
    .unwrap();
    let presenter = fixture.presenter(DialogProcess {
        executable: "/usr/bin/sh".into(),
        arguments: vec![script.into_os_string(), token.clone().into_os_string()],
    });
    let (_cancel, cancelled) = watch::channel(false);
    let pending = tokio::spawn({
        let presenter = presenter.clone();
        async move { presenter.present(request(), cancelled).await }
    });
    let added = receive(&mut fixture.added).await;
    // Signals from a different unique sender cannot satisfy the proxy match.
    fixture
        .client
        .emit_signal(
            None::<&str>,
            PATH,
            "org.freedesktop.impl.portal.Notification",
            "ActionInvoked",
            &(APP, &added.id, "canopa.background.forbid", parameters(0)),
        )
        .await
        .unwrap();
    fixture
        .emit(&added.id, "canopa.background.allow", parameters(0))
        .await;
    assert!(!pending.is_finished());
    let platform = value(HashMap::from([(
        "activation-token".to_owned(),
        value("bound-fixture-token"),
    )]));
    fixture
        .emit(
            &added.id,
            "canopa.background.review",
            vec![3u32.into(), platform],
        )
        .await;
    assert_eq!(pending.await.unwrap().unwrap(), Decision::Allow);
    assert_eq!(
        std::fs::read_to_string(token).unwrap(),
        "bound-fixture-token"
    );
    assert_eq!(receive(&mut fixture.removed).await.id, added.id);
    presenter.shutdown().await.unwrap();
}
