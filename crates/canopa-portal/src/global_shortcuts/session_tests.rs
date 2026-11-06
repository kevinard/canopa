//! Exercise the real D-Bus owner and real framed Rust socket, with controlled UI.
use super::{peer_ended, ready, reply, request, send};
use crate::{
    BUS_NAME, OBJECT_PATH,
    global_shortcuts::{
        self as shortcuts, Action, Actions, Binding, Connector, GlobalShortcuts,
        editor::{self, Decision, Feedback},
        history::History,
    },
    requests::{Options, Requests, Response},
};
use futures_util::{StreamExt, future::BoxFuture};
use serde_json::{Value as Json, json};
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::Arc,
    time::Duration,
};
use tokio::{
    net::UnixStream,
    sync::{mpsc, watch},
};
use zbus::{
    Connection,
    zvariant::{OwnedObjectPath, Value},
};

struct Bus(Child);
impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Source(mpsc::Sender<UnixStream>);
impl Connector for Source {
    fn open<'a>(
        &'a self,
        _: &'a Connection,
    ) -> BoxFuture<'a, anyhow::Result<shortcuts::transport::Lease>> {
        Box::pin(async move {
            let (lease, peer) = ready().await;
            self.0.try_send(peer)?;
            Ok(lease)
        })
    }
}
struct Review {
    request: editor::Request,
    decisions: mpsc::Sender<Decision>,
    feedback: mpsc::Receiver<Feedback>,
    closed: watch::Receiver<bool>,
}
struct Presenter(mpsc::Sender<Review>);
struct Editor {
    events: Option<mpsc::Receiver<Decision>>,
    feedback: mpsc::Sender<Feedback>,
    closed: watch::Sender<bool>,
}
impl Drop for Editor {
    fn drop(&mut self) {
        self.closed.send_replace(true);
    }
}
impl editor::Editor for Editor {
    fn events(&mut self) -> Option<mpsc::Receiver<Decision>> {
        self.events.take()
    }
    fn update(&self, feedback: Feedback) -> anyhow::Result<()> {
        self.feedback.try_send(feedback)?;
        Ok(())
    }
    fn stop(self: Box<Self>) -> BoxFuture<'static, anyhow::Result<()>> {
        Box::pin(async move {
            drop(self);
            Ok(())
        })
    }
}
impl editor::Presenter for Presenter {
    fn open(
        &self,
        request: editor::Request,
    ) -> BoxFuture<'_, anyhow::Result<Box<dyn editor::Editor>>> {
        Box::pin(async move {
            let (decisions, events) = mpsc::channel(4);
            let (feedback, updates) = mpsc::channel(16);
            let (closed, stopped) = watch::channel(false);
            self.0.try_send(Review {
                request,
                decisions,
                feedback: updates,
                closed: stopped,
            })?;
            Ok(Box::new(Editor {
                events: Some(events),
                feedback,
                closed,
            }) as Box<dyn editor::Editor>)
        })
    }
}
struct Fixture {
    _bus: Bus,
    address: String,
    _server: Connection,
    client: Connection,
    portal: GlobalShortcuts,
    reviews: mpsc::Receiver<Review>,
    peers: mpsc::Receiver<UnixStream>,
    history: History,
}
impl Fixture {
    async fn new(history: History) -> Self {
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
        let address = address.trim().to_owned();
        assert!(!address.is_empty(), "Private test bus did not start");
        let (reviews, receive) = mpsc::channel(8);
        let (peers, sources) = mpsc::channel(8);
        let portal = GlobalShortcuts::with_requests(
            Arc::new(Source(peers)),
            Arc::new(Presenter(reviews)),
            history.clone(),
            Requests::default(),
        );
        let server = zbus::connection::Builder::address(address.as_str())
            .unwrap()
            .serve_at(OBJECT_PATH, portal.clone())
            .unwrap()
            .name(BUS_NAME)
            .unwrap()
            .build()
            .await
            .unwrap();
        let client = connect(&address).await;
        Self {
            _bus: bus,
            address,
            _server: server,
            client,
            portal,
            reviews: receive,
            peers: sources,
            history,
        }
    }
    async fn create(&self, name: &str) {
        let response: Response = proxy(&self.client)
            .await
            .call(
                "CreateSession",
                &(
                    handle(&format!("create_{name}")),
                    key(name),
                    "example.App",
                    Options::new(),
                ),
            )
            .await
            .unwrap();
        assert_eq!(response.0, 0);
        assert!(response.1.is_empty());
    }
    async fn incoming(&mut self) -> (Review, UnixStream) {
        let review = tokio::time::timeout(Duration::from_secs(3), self.reviews.recv())
            .await
            .unwrap()
            .unwrap();
        let peer = tokio::time::timeout(Duration::from_secs(3), self.peers.recv())
            .await
            .unwrap()
            .unwrap();
        (review, peer)
    }
    async fn bind(
        &mut self,
        name: &str,
    ) -> (
        Review,
        UnixStream,
        tokio::task::JoinHandle<zbus::Result<Response>>,
    ) {
        let task = tokio::spawn(bind(self.client.clone(), name.to_owned(), actions()));
        let (review, peer) = self.incoming().await;
        (review, peer, task)
    }
    async fn list(&self, name: &str) -> Vec<(String, Options)> {
        let mut response: Response = proxy(&self.client)
            .await
            .call("ListShortcuts", &(handle("list"), key(name)))
            .await
            .unwrap();
        assert_eq!(response.0, 0);
        response.1.remove("shortcuts").unwrap().try_into().unwrap()
    }
    async fn bound(&mut self, name: &str) -> UnixStream {
        let (review, mut peer, binding) = self.bind(name).await;
        apply(&review, 0, "F9").await;
        commit(&mut peer, 1, json!([[1, "F9"]])).await;
        assert_eq!(binding.await.unwrap().unwrap().0, 0);
        peer
    }
}
async fn connect(address: &str) -> Connection {
    zbus::connection::Builder::address(address)
        .unwrap()
        .build()
        .await
        .unwrap()
}
async fn proxy(client: &Connection) -> zbus::Proxy<'_> {
    zbus::Proxy::new(client, BUS_NAME, OBJECT_PATH, shortcuts::INTERFACE)
        .await
        .unwrap()
}
fn handle(name: &str) -> OwnedObjectPath {
    format!("/org/freedesktop/portal/desktop/request/shortcuts/{name}")
        .try_into()
        .unwrap()
}
fn key(name: &str) -> OwnedObjectPath {
    format!("/org/freedesktop/portal/desktop/session/shortcuts/{name}")
        .try_into()
        .unwrap()
}
fn actions() -> Vec<(String, Options)> {
    vec![(
        "capture".into(),
        Options::from([
            (
                "description".into(),
                Value::from("Capture a moment").try_into().unwrap(),
            ),
            (
                "preferred_trigger".into(),
                Value::from("CTRL+a").try_into().unwrap(),
            ),
        ]),
    )]
}
fn saved_actions() -> Actions {
    Actions(vec![Action {
        id: "capture".into(),
        description: "An earlier description".into(),
        preferred_trigger: None,
    }])
}
async fn bind(
    client: Connection,
    name: String,
    actions: Vec<(String, Options)>,
) -> zbus::Result<Response> {
    proxy(&client)
        .await
        .call(
            "BindShortcuts",
            &(
                handle("bind"),
                key(&name),
                actions,
                "wayland:parent",
                Options::new(),
            ),
        )
        .await
}
async fn apply(review: &Review, generation: u64, trigger: &str) {
    review
        .decisions
        .send(Decision::Apply {
            generation,
            bindings: vec![Binding {
                id: "capture".into(),
                trigger: trigger.into(),
            }],
        })
        .await
        .unwrap();
}
async fn commit(peer: &mut UnixStream, generation: u64, bindings: Json) {
    let request = request(peer).await;
    assert_eq!(request["command"]["op"], "replace");
    send(
        peer,
        reply(
            request["sequence"].as_u64().unwrap(),
            generation,
            bindings,
            Json::Null,
            json!([]),
        ),
    )
    .await;
}
async fn configured(f: &mut Fixture, name: &str) -> Review {
    proxy(&f.client)
        .await
        .call::<_, _, ()>(
            "ConfigureShortcuts",
            &(
                key(name),
                "wayland:configured",
                Options::from([(
                    "activation_token".into(),
                    Value::from("input-bound-token").try_into().unwrap(),
                )]),
            ),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), f.reviews.recv())
        .await
        .unwrap()
        .unwrap()
}
async fn wait_closed(mut closed: watch::Receiver<bool>) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !*closed.borrow() {
            closed.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn exact_v2_contract_history_and_canceled_bind_attempt_do_not_install_saved_preferences() {
    let history = History::memory();
    history
        .save(
            "example.App".into(),
            saved_actions(),
            [(1, "F8".into())].into(),
        )
        .await
        .unwrap();
    let mut f = Fixture::new(history).await;
    f.create("one").await;
    assert_eq!(
        proxy(&f.client)
            .await
            .get_property::<u32>("version")
            .await
            .unwrap(),
        2
    );
    let introspection = zbus::Proxy::new(
        &f.client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.DBus.Introspectable",
    )
    .await
    .unwrap();
    let xml: String = introspection.call("Introspect", &()).await.unwrap();
    let configure = xml
        .split("<method name=\"ConfigureShortcuts\">")
        .nth(1)
        .unwrap()
        .split("</method>")
        .next()
        .unwrap();
    assert!(!configure.contains("direction=\"out\""));
    assert!(!configure.contains("name=\"handle\""));
    let list = xml
        .split("<method name=\"ListShortcuts\">")
        .nth(1)
        .unwrap()
        .split("</method>")
        .next()
        .unwrap();
    assert_eq!(list.matches("direction=\"in\"").count(), 2);
    let before = f.list("one").await;
    assert_eq!(before[0].0, "capture");
    assert_eq!(
        String::try_from(
            before[0]
                .1
                .get("trigger_description")
                .unwrap()
                .try_clone()
                .unwrap()
        )
        .unwrap(),
        "F8"
    );
    let (review, mut peer, binding) = f.bind("one").await;
    assert!(!review.request.configuring);
    assert_eq!(review.request.defaults[0].trigger, "F8");
    assert!(review.request.current.is_empty());
    assert_eq!(review.request.parent_window, "wayland:parent");
    assert!(review.request.activation_token.is_none());
    assert!(
        matches!(peer.try_read(&mut [0u8;1]),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock)
    );
    review.decisions.send(Decision::Cancel).await.unwrap();
    assert_eq!(binding.await.unwrap().unwrap().0, 1);
    peer_ended(&mut peer).await;
    assert!(f.list("one").await.is_empty());
    assert!(
        bind(f.client.clone(), "one".into(), actions())
            .await
            .is_err()
    );
    assert!(
        proxy(&f.client)
            .await
            .call::<_, _, ()>("ConfigureShortcuts", &(key("one"), "", Options::new()))
            .await
            .is_err()
    );
    assert_eq!(
        f.history.load("example.App".into()).await.unwrap().bindings[0].trigger,
        "F8"
    );
    f.portal.shutdown().await.unwrap();
}
#[tokio::test]
async fn conflict_retry_stays_in_the_same_editor_and_configure_cancel_keeps_the_current_lease() {
    let mut f = Fixture::new(History::memory()).await;
    f.create("one").await;
    let mut peer = f.bound("one").await;
    let mut review = configured(&mut f, "one").await;
    assert!(review.request.configuring);
    assert_eq!(review.request.current[0].trigger, "F9");
    assert_eq!(
        review.request.activation_token.as_deref(),
        Some("input-bound-token")
    );
    assert!(
        proxy(&f.client)
            .await
            .call::<_, _, ()>("ConfigureShortcuts", &(key("one"), "", Options::new()))
            .await
            .is_err()
    );
    apply(&review, 1, "F10").await;
    let request = request(&mut peer).await;
    assert_eq!(request["command"]["generation"], 1);
    send(
        &mut peer,
        reply(
            request["sequence"].as_u64().unwrap(),
            1,
            json!([[1, "F9"]]),
            json!("conflict"),
            json!([1]),
        ),
    )
    .await;
    let error = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let feedback = review.feedback.recv().await.unwrap();
            if feedback.error.is_some() {
                break feedback;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(error.failed, vec!["capture"]);
    assert_eq!(error.current[0].trigger, "F9");
    assert!(!*review.closed.borrow());
    let listed = f.list("one").await;
    assert_eq!(
        String::try_from(
            listed[0]
                .1
                .get("trigger_description")
                .unwrap()
                .try_clone()
                .unwrap()
        )
        .unwrap(),
        "F9"
    );
    apply(&review, 1, "F11").await;
    commit(&mut peer, 2, json!([[1, "F11"]])).await;
    wait_closed(review.closed).await;
    assert_eq!(
        f.history.load("example.App".into()).await.unwrap().bindings[0].trigger,
        "F11"
    );
    let review = configured(&mut f, "one").await;
    assert_eq!(review.request.current[0].trigger, "F11");
    review.decisions.send(Decision::Cancel).await.unwrap();
    wait_closed(review.closed).await;
    assert_eq!(f.list("one").await.len(), 1);
    assert!(
        matches!(peer.try_read(&mut [0u8;1]),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock)
    );
    assert!(f.peers.try_recv().is_err());
    f.portal.shutdown().await.unwrap();
    peer_ended(&mut peer).await;
}
#[tokio::test]
async fn directed_original_action_signals_keep_u64_source_time_and_configuration_changes_actual_list()
 {
    let mut f = Fixture::new(History::memory()).await;
    f.create("one").await;
    let mut peer = f.bound("one").await;
    let foreign = connect(&f.address).await;
    let other = proxy(&foreign).await;
    let target = proxy(&f.client).await;
    let mut activation = target.receive_signal("Activated").await.unwrap();
    let mut foreign_events = other.receive_signal("Activated").await.unwrap();
    let mut changes = target.receive_signal("ShortcutsChanged").await.unwrap();
    send(
        &mut peer,
        json!({"event":"activated","generation":1,"binding":1,"timestamp_ms":5_000_000_123u64}),
    )
    .await;
    let signal = tokio::time::timeout(Duration::from_secs(3), activation.next())
        .await
        .unwrap()
        .unwrap();
    let (session, id, time, options): (OwnedObjectPath, String, u64, Options) =
        signal.body().deserialize().unwrap();
    assert_eq!(session, key("one"));
    assert_eq!(id, "capture");
    assert_eq!(time, 5_000_000_123);
    assert!(options.is_empty());
    assert!(
        tokio::time::timeout(Duration::from_millis(30), foreign_events.next())
            .await
            .is_err()
    );
    send(
        &mut peer,
        json!({"event":"deactivated","generation":1,"binding":1,"timestamp_ms":5_000_000_124u64}),
    )
    .await;
    send(
        &mut peer,
        json!({"event":"changed","generation":2,"bindings":[],"failed":[1]}),
    )
    .await;
    let signal = tokio::time::timeout(Duration::from_secs(3), changes.next())
        .await
        .unwrap()
        .unwrap();
    let (session, rows): (OwnedObjectPath, Vec<(String, Options)>) =
        signal.body().deserialize().unwrap();
    assert_eq!(session, key("one"));
    assert!(rows.is_empty());
    assert!(f.list("one").await.is_empty());
    assert_eq!(
        f.history.load("example.App".into()).await.unwrap().bindings[0].trigger,
        "F9"
    );
    f.portal.shutdown().await.unwrap();
}
#[tokio::test]
async fn request_close_during_an_unanswered_mutation_revokes_provisional_authority() {
    let mut f = Fixture::new(History::memory()).await;
    f.create("one").await;
    let (review, mut peer, binding) = f.bind("one").await;
    let cancellation = zbus::Proxy::new(
        &f.client,
        BUS_NAME,
        handle("bind"),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap();
    apply(&review, 0, "F9").await;
    let request = request(&mut peer).await;
    assert_eq!(request["command"]["bindings"], json!([[1, "F9"]]));
    cancellation.call::<_, _, ()>("Close", &()).await.unwrap();
    assert_eq!(binding.await.unwrap().unwrap().0, 1);
    peer_ended(&mut peer).await;
    wait_closed(review.closed).await;
    assert!(
        f.history
            .load("example.App".into())
            .await
            .unwrap()
            .bindings
            .is_empty()
    );
    f.portal.shutdown().await.unwrap();
}
#[tokio::test]
async fn foreign_methods_and_close_are_denied_and_original_frontend_disconnect_revokes_the_fd() {
    let mut f = Fixture::new(History::memory()).await;
    f.create("one").await;
    let mut peer = f.bound("one").await;
    let foreign = connect(&f.address).await;
    assert!(
        proxy(&foreign)
            .await
            .call::<_, _, Response>("ListShortcuts", &(handle("foreign"), key("one")))
            .await
            .is_err()
    );
    assert!(
        bind(foreign.clone(), "one".into(), actions())
            .await
            .is_err()
    );
    assert!(
        proxy(&foreign)
            .await
            .call::<_, _, ()>("ConfigureShortcuts", &(key("one"), "", Options::new()))
            .await
            .is_err()
    );
    let session = zbus::Proxy::new(
        &foreign,
        BUS_NAME,
        key("one"),
        "org.freedesktop.impl.portal.Session",
    )
    .await
    .unwrap();
    assert!(session.call::<_, _, ()>("Close", &()).await.is_err());
    assert_eq!(f.list("one").await.len(), 1);
    f.client.clone().close().await.unwrap();
    peer_ended(&mut peer).await;
    f.portal.shutdown().await.unwrap();
}
#[tokio::test]
async fn malformed_first_bind_consumes_the_attempt_and_source_loss_closes_only_its_session() {
    let mut f = Fixture::new(History::memory()).await;
    f.create("one").await;
    f.create("two").await;
    let mut duplicate = actions();
    duplicate.extend(actions());
    assert!(
        bind(f.client.clone(), "one".into(), duplicate)
            .await
            .is_err()
    );
    assert!(
        bind(f.client.clone(), "one".into(), actions())
            .await
            .is_err()
    );
    assert!(f.list("one").await.is_empty());
    let peer = f.bound("two").await;
    let session = zbus::Proxy::new(
        &f.client,
        BUS_NAME,
        key("two"),
        "org.freedesktop.impl.portal.Session",
    )
    .await
    .unwrap();
    let mut closed = session.receive_signal("Closed").await.unwrap();
    drop(peer);
    tokio::time::timeout(Duration::from_secs(3), closed.next())
        .await
        .unwrap()
        .unwrap();
    assert!(f.list("one").await.is_empty());
    assert!(
        proxy(&f.client)
            .await
            .call::<_, _, Response>("ListShortcuts", &(handle("list_dead"), key("two")))
            .await
            .is_err()
    );
    f.portal.shutdown().await.unwrap();
}
#[tokio::test]
async fn empty_explicit_approval_is_a_successful_bind_and_can_be_configured() {
    let mut f = Fixture::new(History::memory()).await;
    f.create("one").await;
    let (review, mut peer, binding) = f.bind("one").await;
    review
        .decisions
        .send(Decision::Apply {
            generation: 0,
            bindings: Vec::new(),
        })
        .await
        .unwrap();
    commit(&mut peer, 1, json!([])).await;
    assert_eq!(binding.await.unwrap().unwrap().0, 0);
    assert!(f.list("one").await.is_empty());
    let review = configured(&mut f, "one").await;
    assert!(review.request.current.is_empty());
    assert!(review.request.configuring);
    review.decisions.send(Decision::Cancel).await.unwrap();
    wait_closed(review.closed).await;
    f.portal.shutdown().await.unwrap();
}

#[tokio::test]
async fn failed_persistence_after_apply_revokes_the_committed_descriptor() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("history.db");
    let mut f = Fixture::new(History::file(path.clone()).unwrap()).await;
    f.create("one").await;
    let (review, mut peer, binding) = f.bind("one").await;
    // The history was absent when consent opened; a source replacement before
    // saving must fail closed after the actual compositor mutation.
    std::os::unix::fs::symlink(directory.path().join("foreign"), path).unwrap();
    apply(&review, 0, "F9").await;
    commit(&mut peer, 1, json!([[1, "F9"]])).await;
    assert!(binding.await.unwrap().is_err());
    peer_ended(&mut peer).await;
    wait_closed(review.closed).await;
    f.portal.shutdown().await.unwrap();
}
#[tokio::test]
async fn session_capacity_recovers_after_explicit_close_without_a_closed_signal() {
    let f = Fixture::new(History::memory()).await;
    for i in 0..16 {
        f.create(&format!("n{i}")).await;
    }
    let response = proxy(&f.client)
        .await
        .call::<_, _, Response>(
            "CreateSession",
            &(
                handle("overflow"),
                key("overflow"),
                "example.App",
                Options::new(),
            ),
        )
        .await;
    assert!(response.is_err());
    let session = zbus::Proxy::new(
        &f.client,
        BUS_NAME,
        key("n0"),
        "org.freedesktop.impl.portal.Session",
    )
    .await
    .unwrap();
    let mut closed = session.receive_signal("Closed").await.unwrap();
    session.call::<_, _, ()>("Close", &()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let response = proxy(&f.client)
                .await
                .call::<_, _, Response>(
                    "CreateSession",
                    &(
                        handle("recovered"),
                        key("recovered"),
                        "example.App",
                        Options::new(),
                    ),
                )
                .await;
            if matches!(response, Ok((0, _))) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(30), closed.next())
            .await
            .is_err()
    );
    f.portal.shutdown().await.unwrap();
}

#[tokio::test]
async fn unexpected_input_descriptors_are_rejected_before_session_creation_or_consent() {
    let f = Fixture::new(History::memory()).await;
    let file = std::fs::File::open("/dev/null").unwrap();
    let options = std::collections::HashMap::from([(
        "future-fd",
        Value::Fd(zbus::zvariant::Fd::from(&file)),
    )]);
    assert!(
        proxy(&f.client)
            .await
            .call::<_, _, Response>(
                "CreateSession",
                &(handle("attached"), key("one"), "example.App", options)
            )
            .await
            .is_err()
    );
    f.create("one").await;
    assert!(f.reviews.is_empty() && f.peers.is_empty());
    f.portal.shutdown().await.unwrap();
}
