use canopa_portal::{
    BUS_NAME, OBJECT_PATH, Settings,
    access::{Access, AccessRequest, Grant},
    account::{Account, AccountRequest, Presenter, UserInformation},
    requests::{Options, Requests, Response},
};
use futures_util::future::BoxFuture;
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
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
    request: AccountRequest,
    reply: oneshot::Sender<anyhow::Result<Option<UserInformation>>>,
}
struct Fake(mpsc::UnboundedSender<Interaction>);
impl Presenter for Fake {
    fn present(
        &self,
        request: AccountRequest,
        _: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<UserInformation>>> {
        Box::pin(async move {
            let (reply, receive) = oneshot::channel();
            self.0.send(Interaction { request, reply }).unwrap();
            receive.await?
        })
    }
}
struct UnusedAccess(Arc<AtomicBool>);
impl canopa_portal::access::Presenter for UnusedAccess {
    fn present(
        &self,
        _: AccessRequest,
        _: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<Grant>>> {
        self.0.store(true, Ordering::SeqCst);
        Box::pin(std::future::pending())
    }
}
fn value(value: impl Into<Value<'static>>) -> OwnedValue {
    OwnedValue::try_from(value.into()).unwrap()
}
fn handle(name: &str) -> OwnedObjectPath {
    format!("/org/freedesktop/portal/desktop/request/account/{name}")
        .try_into()
        .unwrap()
}
fn information(id: &str) -> UserInformation {
    UserInformation {
        id: id.into(),
        name: format!("Name {id}"),
        image: None,
    }
}
async fn proxy(client: &zbus::Connection) -> zbus::Proxy<'_> {
    zbus::Proxy::new(
        client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.Account",
    )
    .await
    .unwrap()
}
async fn call(client: zbus::Connection, name: String, options: Options) -> zbus::Result<Response> {
    proxy(&client)
        .await
        .call(
            "GetUserInformation",
            &(
                handle(&name),
                "example.ProfileApp",
                "wayland:parent",
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
async fn request_proxy<'a>(client: &'a zbus::Connection, name: &str) -> zbus::Proxy<'a> {
    zbus::Proxy::new(
        client,
        BUS_NAME,
        handle(name),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn explicit_sharing_preserves_data_and_request_ownership_without_implicit_disclosure() {
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
    let requests = Requests::default();
    let access_called = Arc::new(AtomicBool::new(false));
    let server = zbus::connection::Builder::address(address)
        .unwrap()
        .serve_at(
            OBJECT_PATH,
            Account::with_requests(Arc::new(Fake(send)), requests.clone()),
        )
        .unwrap()
        .serve_at(
            OBJECT_PATH,
            Access::with_requests(Arc::new(UnusedAccess(access_called.clone())), requests),
        )
        .unwrap()
        .serve_at(OBJECT_PATH, Settings::new(Default::default()))
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

    let introspection = zbus::Proxy::new(
        &client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.DBus.Introspectable",
    )
    .await
    .unwrap();
    let xml: String = introspection.call("Introspect", &()).await.unwrap();
    let account_xml = xml
        .split("<interface name=\"org.freedesktop.impl.portal.Account\">")
        .nth(1)
        .unwrap()
        .split("</interface>")
        .next()
        .unwrap();
    assert!(account_xml.contains("GetUserInformation"));
    assert!(account_xml.contains("name=\"handle\" type=\"o\""));
    assert!(account_xml.contains("name=\"window\" type=\"s\""));
    assert!(!account_xml.contains("<property"));

    let reason = "Share the edited profile with this private fixture.";
    let task = tokio::spawn(call(
        client.clone(),
        "share".into(),
        Options::from([
            ("reason".into(), value(reason)),
            ("future-option".into(), true.into()),
        ]),
    ));
    let request = incoming(&mut interactions).await;
    assert_eq!(request.request.app_id, "example.ProfileApp");
    assert_eq!(request.request.parent_window, "wayland:parent");
    assert_eq!(request.request.reason, reason);
    assert!(!task.is_finished(), "opening cannot disclose information");
    request
        .reply
        .send(Ok(Some(UserInformation {
            id: "edited-alias".into(),
            name: "Edited <b>name</b>".into(),
            image: Some("file:///tmp/raw-%FF.png".into()),
        })))
        .unwrap();
    let (status, mut result) = task.await.unwrap().unwrap();
    assert_eq!(status, 0);
    assert_eq!(result.len(), 3);
    assert_eq!(
        String::try_from(result.remove("id").unwrap()).unwrap(),
        "edited-alias"
    );
    assert_eq!(
        String::try_from(result.remove("name").unwrap()).unwrap(),
        "Edited <b>name</b>"
    );
    assert_eq!(
        String::try_from(result.remove("image").unwrap()).unwrap(),
        "file:///tmp/raw-%FF.png"
    );

    let first = tokio::spawn(call(client.clone(), "first".into(), Options::new()));
    let first_request = incoming(&mut interactions).await;
    let second = tokio::spawn(call(client.clone(), "second".into(), Options::new()));
    let second_request = incoming(&mut interactions).await;
    second_request
        .reply
        .send(Ok(Some(information("second"))))
        .unwrap();
    assert_eq!(
        String::try_from(second.await.unwrap().unwrap().1.remove("id").unwrap()).unwrap(),
        "second"
    );
    assert!(!first.is_finished());
    first_request
        .reply
        .send(Ok(Some(UserInformation::default())))
        .unwrap();
    let (status, values) = first.await.unwrap().unwrap();
    assert_eq!(status, 0);
    assert_eq!(values.len(), 3);
    for value in values.into_values() {
        assert_eq!(String::try_from(value).unwrap(), "");
    }

    for (name, result) in [
        ("cancel", Ok(None)),
        ("failed", Err(anyhow::anyhow!("private presenter error"))),
    ] {
        let expected = if result.is_ok() { 1 } else { 2 };
        let task = tokio::spawn(call(client.clone(), name.into(), Options::new()));
        incoming(&mut interactions)
            .await
            .reply
            .send(result)
            .unwrap();
        let (status, values) = task.await.unwrap().unwrap();
        assert_eq!(status, expected);
        assert!(values.is_empty());
    }
    for (index, information) in [
        UserInformation {
            id: "private\0id".into(),
            ..information("invalid")
        },
        UserInformation {
            name: "x".repeat(4097),
            ..information("invalid")
        },
        UserInformation {
            image: Some("https://example.test/profile.png".into()),
            ..information("invalid")
        },
        UserInformation {
            image: Some("file:///tmp/photo%00.png".into()),
            ..information("invalid")
        },
        UserInformation {
            image: Some("file:///tmp/photo.png?read=other".into()),
            ..information("invalid")
        },
    ]
    .into_iter()
    .enumerate()
    {
        let task = tokio::spawn(call(
            client.clone(),
            format!("invalid_{index}"),
            Options::new(),
        ));
        incoming(&mut interactions)
            .await
            .reply
            .send(Ok(Some(information)))
            .unwrap();
        let (status, values) = task.await.unwrap().unwrap();
        assert_eq!(status, 2);
        assert!(values.is_empty());
    }

    let task = tokio::spawn(call(client.clone(), "close".into(), Options::new()));
    let pending = incoming(&mut interactions).await;
    assert!(
        matches!(request_proxy(&stranger, "close").await.call::<_, _, ()>("Close", &()).await,
        Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.AccessDenied")
    );
    assert!(!pending.reply.is_closed());
    request_proxy(&client, "close")
        .await
        .call::<_, _, ()>("Close", &())
        .await
        .unwrap();
    let (status, values) = task.await.unwrap().unwrap();
    assert_eq!(status, 1);
    assert!(values.is_empty());
    assert!(pending.reply.is_closed());
    assert!(
        request_proxy(&client, "close")
            .await
            .call::<_, _, ()>("Close", &())
            .await
            .is_err()
    );

    let lost = zbus::connection::Builder::address(address)
        .unwrap()
        .build()
        .await
        .unwrap();
    let task = tokio::spawn(call(lost.clone(), "lost".into(), Options::new()));
    let mut pending = incoming(&mut interactions).await;
    lost.close().await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), pending.reply.closed())
        .await
        .unwrap();
    let _ = task.await;
    for options in [
        Options::from([("reason".into(), false.into())]),
        Options::from([("reason".into(), value("é".repeat(257)))]),
    ] {
        assert!(call(client.clone(), "bad".into(), options).await.is_err());
        assert!(interactions.try_recv().is_err());
    }
    let task = tokio::spawn(call(
        client.clone(),
        "unicode".into(),
        Options::from([("reason".into(), value("é".repeat(256)))]),
    ));
    let pending = incoming(&mut interactions).await;
    assert_eq!(pending.request.reason.chars().count(), 256);
    pending.reply.send(Ok(None)).unwrap();
    assert_eq!(task.await.unwrap().unwrap().0, 1);

    let mut tasks = Vec::new();
    let mut pending = Vec::new();
    for index in 0..16 {
        tasks.push(tokio::spawn(call(
            client.clone(),
            format!("slot_{index}"),
            Options::new(),
        )));
        pending.push(incoming(&mut interactions).await);
    }
    assert!(
        matches!(call(client.clone(), "overflow".into(), Options::new()).await,
        Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.LimitsExceeded")
    );
    let access = zbus::Proxy::new(
        &client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.Access",
    )
    .await
    .unwrap();
    let overflow: zbus::Result<Response> = access
        .call(
            "AccessDialog",
            &(
                handle("access_overflow"),
                "example.App",
                "",
                "Question",
                "",
                "",
                Options::new(),
            ),
        )
        .await;
    assert!(matches!(overflow, Err(zbus::Error::MethodError(name, _, _))
        if name.as_str() == "org.freedesktop.DBus.Error.LimitsExceeded"));
    assert!(!access_called.load(Ordering::SeqCst));
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
    for request in pending {
        request.reply.send(Ok(None)).unwrap();
    }
    for task in tasks {
        assert_eq!(task.await.unwrap().unwrap().0, 1);
    }
    server.close().await.unwrap();
}
