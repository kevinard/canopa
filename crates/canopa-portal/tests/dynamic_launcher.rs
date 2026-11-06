use canopa_portal::{
    BUS_NAME, OBJECT_PATH, Settings,
    access::{Access, AccessRequest, Grant},
    dynamic_launcher::{DynamicLauncher, LauncherRequest, LauncherType, Presenter},
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
    request: LauncherRequest,
    reply: oneshot::Sender<anyhow::Result<Option<String>>>,
}
struct Fake(mpsc::UnboundedSender<Interaction>);
impl Presenter for Fake {
    fn present(
        &self,
        request: LauncherRequest,
        _: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<String>>> {
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
impl canopa_portal::background::Presenter for UnusedAccess {
    fn present(
        &self,
        _: canopa_portal::background::BackgroundRequest,
        _: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<canopa_portal::background::Decision>> {
        Box::pin(std::future::pending())
    }
}
fn value(value: impl Into<Value<'static>>) -> OwnedValue {
    value.into().try_into_owned().unwrap()
}
fn icon() -> OwnedValue {
    value(("bytes", Value::from(vec![17_u8, 42, 73])))
}
fn handle(name: &str) -> OwnedObjectPath {
    format!("/org/freedesktop/portal/desktop/request/launcher/{name}")
        .try_into()
        .unwrap()
}
async fn proxy(client: &zbus::Connection) -> zbus::Proxy<'_> {
    zbus::Proxy::new(
        client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.DynamicLauncher",
    )
    .await
    .unwrap()
}
async fn call(client: zbus::Connection, name: String, options: Options) -> zbus::Result<Response> {
    proxy(&client)
        .await
        .call(
            "PrepareInstall",
            &(
                handle(&name),
                "org.canopa.Fixture",
                "wayland:parent",
                "Original",
                icon(),
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
async fn close(client: &zbus::Connection, name: &str) -> zbus::Result<()> {
    zbus::Proxy::new(
        client,
        BUS_NAME,
        handle(name),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap()
    .call("Close", &())
    .await
}

#[tokio::test]
async fn typed_review_results_and_shared_request_lifetimes_are_preserved() {
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
    let process = Arc::new(canopa_portal::process::DialogProcess {
        executable: "/unused/introspection-only".into(),
        arguments: Vec::new(),
    });
    let server = zbus::connection::Builder::address(address)
        .unwrap()
        .serve_at(
            OBJECT_PATH,
            DynamicLauncher::with_requests(Arc::new(Fake(send)), requests.clone()),
        )
        .unwrap()
        .serve_at(
            OBJECT_PATH,
            Access::with_requests(
                Arc::new(UnusedAccess(access_called.clone())),
                requests.clone(),
            ),
        )
        .unwrap()
        .serve_at(
            OBJECT_PATH,
            canopa_portal::file_chooser::FileChooser::with_requests(
                process.clone(),
                requests.clone(),
            ),
        )
        .unwrap()
        .serve_at(
            OBJECT_PATH,
            canopa_portal::app_chooser::AppChooser::with_requests(
                process.clone(),
                requests.clone(),
            ),
        )
        .unwrap()
        .serve_at(
            OBJECT_PATH,
            canopa_portal::account::Account::with_requests(process, requests.clone()),
        )
        .unwrap()
        .serve_at(
            OBJECT_PATH,
            canopa_portal::background::Background::with_requests(
                Arc::new(UnusedAccess(access_called.clone())),
                watch::channel(canopa_portal::background::Observation::unavailable(
                    canopa_portal::background::Unavailable::Disconnected,
                ))
                .1,
                requests,
            ),
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
    let dynamic = proxy(&client).await;
    assert_eq!(
        dynamic
            .get_property::<u32>("SupportedLauncherTypes")
            .await
            .unwrap(),
        3
    );
    assert_eq!(dynamic.get_property::<u32>("version").await.unwrap(), 1);
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
    for (interface, methods) in [
        ("FileChooser", vec!["OpenFile", "SaveFile", "SaveFiles"]),
        ("AppChooser", vec!["ChooseApplication"]),
        ("Access", vec!["AccessDialog"]),
        ("Account", vec!["GetUserInformation"]),
        ("Background", vec!["NotifyBackground"]),
    ] {
        let interface = xml
            .split(&format!(
                "<interface name=\"org.freedesktop.impl.portal.{interface}\">"
            ))
            .nth(1)
            .unwrap()
            .split("</interface>")
            .next()
            .unwrap();
        for method in methods {
            let method = interface
                .split(&format!("<method name=\"{method}\">"))
                .nth(1)
                .unwrap()
                .split("</method>")
                .next()
                .unwrap();
            assert!(
                method.contains("name=\"response\" type=\"u\" direction=\"out\""),
                "{method}"
            );
            assert!(
                method.contains("name=\"results\" type=\"a{sv}\" direction=\"out\""),
                "{method}"
            );
            assert_eq!(method.matches("direction=\"out\"").count(), 2, "{method}");
        }
    }
    let xml = xml
        .split("<interface name=\"org.freedesktop.impl.portal.DynamicLauncher\">")
        .nth(1)
        .unwrap()
        .split("</interface>")
        .next()
        .unwrap();
    for field in [
        "name=\"handle\" type=\"o\"",
        "name=\"app_id\" type=\"s\"",
        "name=\"parent_window\" type=\"s\"",
        "name=\"icon_v\" type=\"v\"",
        "name=\"options\" type=\"a{sv}\"",
        "name=\"response\" type=\"u\"",
        "name=\"results\" type=\"a{sv}\"",
    ] {
        assert!(xml.contains(field), "{field}: {xml}");
    }
    assert!(xml.contains("RequestInstallToken"));
    for app_id in ["org.canopa.Fixture", "org.gnome.Software", ""] {
        let status: u32 = dynamic
            .call("RequestInstallToken", &(app_id, Options::new()))
            .await
            .unwrap();
        assert_eq!(status, 2);
    }
    assert!(interactions.try_recv().is_err());

    let task = tokio::spawn(call(
        client.clone(),
        "approve".into(),
        Options::from([
            ("launcher_type".into(), 2_u32.into()),
            (
                "target".into(),
                value("https://example.test/install?q=one#two"),
            ),
            ("modal".into(), false.into()),
            ("editable_icon".into(), true.into()),
        ]),
    ));
    let pending = incoming(&mut interactions).await;
    assert_eq!(pending.request.draft.app_id, "org.canopa.Fixture");
    assert_eq!(pending.request.draft.parent_window, "wayland:parent");
    assert_eq!(pending.request.draft.name, "Original");
    assert_eq!(pending.request.draft.launcher_type, LauncherType::Webapp);
    assert_eq!(
        pending.request.draft.target,
        "https://example.test/install?q=one#two"
    );
    assert!(!pending.request.draft.modal);
    assert_eq!(pending.request.icon.bytes(), [17, 42, 73]);
    assert!(!task.is_finished());
    pending
        .reply
        .send(Ok(Some("Edited <b>name</b>".into())))
        .unwrap();
    let (status, mut result) = task.await.unwrap().unwrap();
    assert_eq!(status, 0);
    assert_eq!(result.len(), 2);
    assert_eq!(
        String::try_from(result.remove("name").unwrap()).unwrap(),
        "Edited <b>name</b>"
    );
    let returned_icon = result.remove("icon").unwrap();
    assert_eq!(returned_icon.value_signature().to_string(), "v");
    let Value::Value(serialized) = Value::from(returned_icon) else {
        panic!("missing nested variant")
    };
    assert_eq!(
        canopa_portal::dynamic_launcher::LauncherIcon::parse(serialized.try_into_owned().unwrap())
            .unwrap()
            .bytes(),
        [17, 42, 73]
    );

    for (index, answer, options, expected) in [
        (0, Ok(None), Options::new(), 1),
        (
            1,
            Err(anyhow::anyhow!("private failure")),
            Options::new(),
            2,
        ),
        (2, Ok(Some("".into())), Options::new(), 2),
        (
            3,
            Ok(Some("Changed".into())),
            Options::from([("editable_name".into(), false.into())]),
            2,
        ),
        (
            4,
            Ok(Some("Original".into())),
            Options::from([("editable_name".into(), false.into())]),
            0,
        ),
    ] {
        let task = tokio::spawn(call(client.clone(), format!("result{index}"), options));
        incoming(&mut interactions)
            .await
            .reply
            .send(answer)
            .unwrap();
        let (status, result) = task.await.unwrap().unwrap();
        assert_eq!(status, expected);
        assert_eq!(result.is_empty(), expected != 0);
    }
    for bad_icon in [
        value(("bytes", Value::from(vec![1_u8]), 5_u32)),
        value(("file", Value::from("file:///private"))),
    ] {
        let result: zbus::Result<Response> = dynamic
            .call(
                "PrepareInstall",
                &(handle("bad"), "app", "", "Name", bad_icon, Options::new()),
            )
            .await;
        assert!(
            matches!(result, Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.InvalidArgs")
        );
        assert!(interactions.try_recv().is_err());
    }
    assert!(
        call(
            client.clone(),
            "bad_option".into(),
            Options::from([("launcher_type".into(), 3_u32.into())])
        )
        .await
        .is_err()
    );
    assert!(interactions.try_recv().is_err());

    let first = tokio::spawn(call(client.clone(), "first".into(), Options::new()));
    let one = incoming(&mut interactions).await;
    let second = tokio::spawn(call(client.clone(), "second".into(), Options::new()));
    let two = incoming(&mut interactions).await;
    two.reply.send(Ok(Some("Second".into()))).unwrap();
    assert_eq!(
        String::try_from(second.await.unwrap().unwrap().1.remove("name").unwrap()).unwrap(),
        "Second"
    );
    assert!(!first.is_finished());
    one.reply.send(Ok(Some("First".into()))).unwrap();
    assert_eq!(
        String::try_from(first.await.unwrap().unwrap().1.remove("name").unwrap()).unwrap(),
        "First"
    );

    let task = tokio::spawn(call(client.clone(), "close".into(), Options::new()));
    let pending = incoming(&mut interactions).await;
    assert!(
        matches!(close(&stranger, "close").await, Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.AccessDenied")
    );
    assert!(!pending.reply.is_closed());
    close(&client, "close").await.unwrap();
    assert_eq!(task.await.unwrap().unwrap().0, 1);
    assert!(pending.reply.is_closed());
    assert!(close(&client, "close").await.is_err());

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

    let mut tasks = Vec::new();
    let mut pending = Vec::new();
    for index in 0..16 {
        tasks.push(tokio::spawn(call(
            client.clone(),
            format!("slot{index}"),
            Options::new(),
        )));
        pending.push(incoming(&mut interactions).await);
    }
    assert!(
        matches!(call(client.clone(), "overflow".into(), Options::new()).await, Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.LimitsExceeded")
    );
    let overflow: zbus::Result<Response> = zbus::Proxy::new(
        &client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.Access",
    )
    .await
    .unwrap()
    .call(
        "AccessDialog",
        &(
            handle("access_overflow"),
            "app",
            "",
            "Question",
            "",
            "",
            Options::new(),
        ),
    )
    .await;
    assert!(
        matches!(overflow, Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.LimitsExceeded")
    );
    assert!(!access_called.load(Ordering::SeqCst));
    let settings: std::collections::HashMap<String, Options> = zbus::Proxy::new(
        &client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.Settings",
    )
    .await
    .unwrap()
    .call("ReadAll", &(Vec::<String>::new(),))
    .await
    .unwrap();
    assert!(!settings.is_empty());
    for request in pending {
        request.reply.send(Ok(None)).unwrap();
    }
    for task in tasks {
        assert_eq!(task.await.unwrap().unwrap().0, 1);
    }
    server.close().await.unwrap();
}
