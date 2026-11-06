use canopa_portal::{
    BUS_NAME, OBJECT_PATH, Settings,
    background::{
        Activity, AppStates, Background, BackgroundRequest, Decision, Observation, Presenter,
        Unavailable,
    },
    requests::{Options, Requests, Response},
};
use futures_util::{StreamExt, future::BoxFuture};
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, watch};
use zbus::zvariant::OwnedObjectPath;

struct Bus(Child);
impl Drop for Bus {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}
struct Question {
    request: BackgroundRequest,
    reply: oneshot::Sender<anyhow::Result<Decision>>,
}
struct Fake(mpsc::UnboundedSender<Question>);
impl Presenter for Fake {
    fn present(
        &self,
        request: BackgroundRequest,
        _: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Decision>> {
        Box::pin(async move {
            let (reply, result) = oneshot::channel();
            self.0.send(Question { request, reply }).unwrap();
            result.await?
        })
    }
}
fn handle(name: &str) -> OwnedObjectPath {
    format!("/org/freedesktop/portal/desktop/notify/background_{name}")
        .try_into()
        .unwrap()
}
async fn proxy(client: &zbus::Connection) -> zbus::Proxy<'_> {
    zbus::Proxy::new(
        client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.Background",
    )
    .await
    .unwrap()
}
async fn call(client: zbus::Connection, name: String) -> zbus::Result<Response> {
    proxy(&client)
        .await
        .call(
            "NotifyBackground",
            &(handle(&name), "org.canopa.BackgroundFixture", name),
        )
        .await
}
async fn question(queue: &mut mpsc::UnboundedReceiver<Question>) -> Question {
    tokio::time::timeout(Duration::from_secs(3), queue.recv())
        .await
        .unwrap()
        .unwrap()
}
fn decision(response: Response, expected: Decision) {
    assert_eq!(response.1.len(), 1);
    assert_eq!(
        u32::try_from(response.1["result"].try_clone().unwrap()).unwrap(),
        expected as u32
    );
}

#[tokio::test]
async fn complete_observation_and_explicit_decisions_preserve_frontend_policy_and_request_ownership()
 {
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
    let (send, mut received) = mpsc::unbounded_channel();
    let (source, state) = watch::channel(Observation::unavailable(Unavailable::Connecting));
    let background = Background::with_requests(Arc::new(Fake(send)), state, Requests::default());
    let changes = background.changes();
    let server = zbus::connection::Builder::address(address.trim())
        .unwrap()
        .serve_at(OBJECT_PATH, background)
        .unwrap()
        .serve_at(OBJECT_PATH, Settings::new(Default::default()))
        .unwrap()
        .name(BUS_NAME)
        .unwrap()
        .build()
        .await
        .unwrap();
    let relay = tokio::spawn(Background::relay_changes(changes, server.clone()));
    let client = zbus::connection::Builder::address(address.trim())
        .unwrap()
        .build()
        .await
        .unwrap();
    let stranger = zbus::connection::Builder::address(address.trim())
        .unwrap()
        .build()
        .await
        .unwrap();
    let bg = proxy(&client).await;
    let xml: String = zbus::Proxy::new(
        &client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.DBus.Introspectable",
    )
    .await
    .unwrap()
    .call("Introspect", &())
    .await
    .unwrap();
    let interface = xml
        .split("<interface name=\"org.freedesktop.impl.portal.Background\">")
        .nth(1)
        .unwrap()
        .split("</interface>")
        .next()
        .unwrap();
    for method in [
        "GetAppState",
        "NotifyBackground",
        "EnableAutostart",
        "RunningApplicationsChanged",
    ] {
        assert!(interface.contains(method), "{method}");
    }
    assert!(!interface.contains("<property"));
    let absent: zbus::Result<Options> = bg.call("GetAppState", &()).await;
    assert!(absent.is_err());
    let mut changed = bg
        .receive_signal("RunningApplicationsChanged")
        .await
        .unwrap();
    source.send_replace(Observation::available(AppStates::from([
        ("org.canopa.One".into(), Activity::Running),
        ("org.canopa.Two".into(), Activity::Active),
    ])));
    tokio::time::timeout(Duration::from_secs(3), changed.next())
        .await
        .unwrap()
        .unwrap()
        .body()
        .deserialize::<()>()
        .unwrap();
    let apps: Options = bg.call("GetAppState", &()).await.unwrap();
    assert_eq!(apps.len(), 2);
    assert_eq!(
        u32::try_from(apps["org.canopa.One"].try_clone().unwrap()).unwrap(),
        1
    );
    assert_eq!(
        u32::try_from(apps["org.canopa.Two"].try_clone().unwrap()).unwrap(),
        2
    );
    source.send_replace(Observation::unavailable(Unavailable::Identity));
    tokio::time::timeout(Duration::from_secs(3), changed.next())
        .await
        .unwrap()
        .unwrap();
    let unavailable: zbus::Result<Options> = bg.call("GetAppState", &()).await;
    assert!(unavailable.is_err());
    source.send_replace(Observation::available(AppStates::new()));
    let apps: Options = bg.call("GetAppState", &()).await.unwrap();
    assert!(apps.is_empty());
    let legacy: zbus::Result<bool> = bg
        .call(
            "EnableAutostart",
            &("org.canopa.One", true, vec!["/usr/bin/true"], 0u32),
        )
        .await;
    assert!(legacy.unwrap_err().to_string().contains("NotSupported"));

    for choice in [Decision::Forbid, Decision::Allow, Decision::AllowOnce] {
        let pending = tokio::spawn(call(client.clone(), format!("choice{}", choice as u32)));
        let interaction = question(&mut received).await;
        assert_eq!(interaction.request.app_id, "org.canopa.BackgroundFixture");
        interaction.reply.send(Ok(choice)).unwrap();
        let result = pending.await.unwrap().unwrap();
        assert_eq!(result.0, 0);
        decision(result, choice);
    }
    let first = tokio::spawn(call(client.clone(), "first".into()));
    let second = tokio::spawn(call(client.clone(), "second".into()));
    let a = question(&mut received).await;
    let b = question(&mut received).await;
    let (a, b) = if a.request.name == "first" {
        (a, b)
    } else {
        (b, a)
    };
    b.reply.send(Ok(Decision::AllowOnce)).unwrap();
    decision(second.await.unwrap().unwrap(), Decision::AllowOnce);
    assert!(!first.is_finished());
    a.reply.send(Ok(Decision::Forbid)).unwrap();
    decision(first.await.unwrap().unwrap(), Decision::Forbid);

    let pending = tokio::spawn(call(client.clone(), "close".into()));
    let late = question(&mut received).await;
    let request = zbus::Proxy::new(
        &stranger,
        BUS_NAME,
        handle("close"),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap();
    let denied: zbus::Result<()> = request.call("Close", &()).await;
    assert!(denied.unwrap_err().to_string().contains("AccessDenied"));
    let request = zbus::Proxy::new(
        &client,
        BUS_NAME,
        handle("close"),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap();
    let _: () = request.call("Close", &()).await.unwrap();
    decision(pending.await.unwrap().unwrap(), Decision::AllowOnce);
    assert!(late.reply.send(Ok(Decision::Allow)).is_err());
    let retired: zbus::Result<()> = request.call("Close", &()).await;
    assert!(retired.is_err());

    let lost = zbus::connection::Builder::address(address.trim())
        .unwrap()
        .build()
        .await
        .unwrap();
    let pending = tokio::spawn(call(lost.clone(), "lost".into()));
    let late = question(&mut received).await;
    lost.close().await.unwrap();
    let _ = pending.await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while !late.reply.is_closed() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(late.reply.send(Ok(Decision::Forbid)).is_err());
    let failed = tokio::spawn(call(client.clone(), "failure".into()));
    question(&mut received)
        .await
        .reply
        .send(Err(anyhow::anyhow!("fixture presentation failed")))
        .unwrap();
    decision(failed.await.unwrap().unwrap(), Decision::AllowOnce);

    for (id, name) in [
        ("bad/id".to_owned(), "Name".to_owned()),
        ("org.canopa.Fixture".into(), "x".repeat(4097)),
    ] {
        let invalid: zbus::Result<Response> = bg
            .call("NotifyBackground", &(handle("invalid"), id, name))
            .await;
        assert!(invalid.unwrap_err().to_string().contains("InvalidArgs"));
    }
    for path in [
        OBJECT_PATH,
        "/org/freedesktop/portal/desktop/notify/background",
        "/org/freedesktop/portal/desktop/notify/background1/nested",
        "/org/freedesktop/portal/desktop/notify/other1",
    ] {
        let invalid: zbus::Result<Response> = bg
            .call(
                "NotifyBackground",
                &(
                    OwnedObjectPath::try_from(path).unwrap(),
                    "org.canopa.BackgroundFixture",
                    "Invalid path",
                ),
            )
            .await;
        assert!(invalid.unwrap_err().to_string().contains("InvalidArgs"));
    }
    assert!(received.try_recv().is_err());
    let mut pending = Vec::new();
    let mut replies = Vec::new();
    for n in 0..16 {
        pending.push(tokio::spawn(call(client.clone(), format!("full{n}"))));
        replies.push(question(&mut received).await.reply);
    }
    let overflow = call(client.clone(), "overflow".into()).await.unwrap_err();
    assert!(overflow.to_string().contains("LimitsExceeded"));
    let settings = zbus::Proxy::new(
        &client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.Settings",
    )
    .await
    .unwrap();
    let values: std::collections::HashMap<String, Options> = settings
        .call("ReadAll", &(Vec::<String>::new(),))
        .await
        .unwrap();
    assert!(!values.is_empty());
    for reply in replies {
        reply.send(Ok(Decision::AllowOnce)).unwrap();
    }
    for call in pending {
        decision(call.await.unwrap().unwrap(), Decision::AllowOnce);
    }
    relay.abort();
    server.close().await.unwrap();
}
