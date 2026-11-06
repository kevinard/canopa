use canopa_portal::{
    BUS_NAME, OBJECT_PATH,
    access::{Access, AccessRequest, Grant, Presenter},
    requests::{Options, Response},
};
use futures_util::future::BoxFuture;
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, watch};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

struct Bus(Child);
impl Drop for Bus {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}
struct Interaction {
    request: AccessRequest,
    reply: oneshot::Sender<Option<Grant>>,
}
struct Fake(mpsc::UnboundedSender<Interaction>);
impl Presenter for Fake {
    fn present(
        &self,
        request: AccessRequest,
        _: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<Grant>>> {
        Box::pin(async move {
            let (reply, receive) = oneshot::channel();
            self.0.send(Interaction { request, reply }).unwrap();
            Ok(receive.await?)
        })
    }
}
fn value(value: impl Into<Value<'static>>) -> OwnedValue {
    OwnedValue::try_from(value.into()).unwrap()
}
fn handle(name: &str) -> OwnedObjectPath {
    format!("/org/freedesktop/portal/desktop/request/access/{name}")
        .try_into()
        .unwrap()
}
async fn proxy(client: &zbus::Connection) -> zbus::Proxy<'_> {
    zbus::Proxy::new(
        client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.Access",
    )
    .await
    .unwrap()
}
async fn call(
    client: zbus::Connection,
    name: &'static str,
    options: Options,
) -> zbus::Result<Response> {
    proxy(&client)
        .await
        .call(
            "AccessDialog",
            &(
                handle(name),
                "example.App",
                "wayland:parent",
                "Camera permission",
                "Application wants to use the camera",
                "Only grant if you trust this application.",
                options,
            ),
        )
        .await
}
async fn incoming(receiver: &mut mpsc::UnboundedReceiver<Interaction>) -> Interaction {
    tokio::time::timeout(Duration::from_secs(3), receiver.recv())
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn consent_requires_a_grant_and_owned_requests_retire_without_one() {
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
    let address = address.trim();
    let (send, mut interactions) = mpsc::unbounded_channel();
    let server = zbus::connection::Builder::address(address)
        .unwrap()
        .serve_at(OBJECT_PATH, Access::new(Arc::new(Fake(send))))
        .unwrap()
        .name(BUS_NAME)
        .unwrap()
        .build()
        .await
        .unwrap();
    let client = zbus::connection::Builder::address(address)
        .unwrap()
        .build()
        .await
        .unwrap();
    let stranger = zbus::connection::Builder::address(address)
        .unwrap()
        .build()
        .await
        .unwrap();
    let choices = vec![
        (
            "remember".to_owned(),
            "Remember".to_owned(),
            vec![],
            "false".to_owned(),
        ),
        (
            "scope".to_owned(),
            "Scope".to_owned(),
            vec![
                ("once".to_owned(), "Once".to_owned()),
                ("session".to_owned(), "Session".to_owned()),
            ],
            "once".to_owned(),
        ),
    ];
    let options = Options::from([
        ("choices".into(), value(choices)),
        ("modal".into(), false.into()),
        ("deny_label".into(), value("Refuse")),
        ("grant_label".into(), value("Permit")),
        ("icon".into(), value("camera-web-symbolic")),
    ]);
    let task = tokio::spawn(call(client.clone(), "grant", options));
    let request = incoming(&mut interactions).await;
    assert_eq!(request.request.app_id, "example.App");
    assert_eq!(request.request.parent_window, "wayland:parent");
    assert_eq!(request.request.title, "Camera permission");
    assert_eq!(
        request.request.subtitle,
        "Application wants to use the camera"
    );
    assert_eq!(
        request.request.body,
        "Only grant if you trust this application."
    );
    assert!(!request.request.modal);
    assert_eq!(request.request.deny_label, "Refuse");
    assert_eq!(request.request.grant_label, "Permit");
    assert_eq!(request.request.icon.as_deref(), Some("camera-web-symbolic"));
    request
        .reply
        .send(Some(Grant {
            choices: vec![
                ("remember".into(), "true".into()),
                ("scope".into(), "session".into()),
            ],
        }))
        .unwrap();
    let (status, mut result) = task.await.unwrap().unwrap();
    assert_eq!(status, 0);
    assert_eq!(
        Vec::<(String, String)>::try_from(result.remove("choices").unwrap()).unwrap(),
        [
            ("remember".into(), "true".into()),
            ("scope".into(), "session".into())
        ]
    );

    let task = tokio::spawn(call(client.clone(), "deny", Options::new()));
    let request = incoming(&mut interactions).await;
    assert!(request.request.modal);
    request.reply.send(None).unwrap();
    let (status, result) = task.await.unwrap().unwrap();
    assert_eq!(status, 1);
    assert!(result.is_empty());

    let task = tokio::spawn(call(client.clone(), "forged", Options::new()));
    incoming(&mut interactions)
        .await
        .reply
        .send(Some(Grant {
            choices: vec![("not-requested".into(), "true".into())],
        }))
        .unwrap();
    assert_eq!(task.await.unwrap().unwrap().0, 2);

    let task = tokio::spawn(call(client.clone(), "close", Options::new()));
    let pending = incoming(&mut interactions).await;
    let wrong = zbus::Proxy::new(
        &stranger,
        BUS_NAME,
        handle("close"),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap();
    assert!(
        matches!(wrong.call::<_, _, ()>("Close", &()).await, Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.AccessDenied")
    );
    assert!(!pending.reply.is_closed());
    let own = zbus::Proxy::new(
        &client,
        BUS_NAME,
        handle("close"),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap();
    own.call::<_, _, ()>("Close", &()).await.unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .0,
        1
    );
    assert!(pending.reply.is_closed());
    assert!(own.call::<_, _, ()>("Close", &()).await.is_err());

    // Caller loss cancels a presenter that deliberately ignores its watch input.
    let lost = zbus::connection::Builder::address(address)
        .unwrap()
        .build()
        .await
        .unwrap();
    let task = tokio::spawn(call(lost.clone(), "lost", Options::new()));
    let mut pending = incoming(&mut interactions).await;
    lost.close().await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), pending.reply.closed())
        .await
        .unwrap();
    let _ = task.await;
    assert!(
        call(
            client.clone(),
            "bad",
            Options::from([("modal".into(), value("yes"))])
        )
        .await
        .is_err()
    );
    assert!(
        call(
            client,
            "bad",
            Options::from([(
                "choices".into(),
                value(vec![
                    (
                        "duplicate".to_owned(),
                        "One".to_owned(),
                        Vec::<(String, String)>::new(),
                        "false".to_owned()
                    ),
                    ("duplicate".into(), "Two".into(), vec![], "true".into())
                ])
            )])
        )
        .await
        .is_err()
    );
    server.close().await.unwrap();
}
