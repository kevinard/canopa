use canopa_portal::{
    BUS_NAME, OBJECT_PATH,
    requests::{Options, Response},
    usb::{Grant, Presenter, SelectedDevice, Usb, UsbRequest},
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
    request: UsbRequest,
    reply: oneshot::Sender<anyhow::Result<Option<Grant>>>,
}
struct Fake(mpsc::UnboundedSender<Interaction>);
impl Presenter for Fake {
    fn present(
        &self,
        request: UsbRequest,
        _: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<Grant>>> {
        Box::pin(async move {
            let (reply, receive) = oneshot::channel();
            self.0.send(Interaction { request, reply }).unwrap();
            receive.await?
        })
    }
}
struct Fixture {
    _bus: Bus,
    address: String,
    server: zbus::Connection,
    client: zbus::Connection,
    interactions: mpsc::UnboundedReceiver<Interaction>,
}
impl Fixture {
    async fn new() -> Self {
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
        let (send, interactions) = mpsc::unbounded_channel();
        let server = zbus::connection::Builder::address(address.as_str())
            .unwrap()
            .serve_at(OBJECT_PATH, Usb::new(Arc::new(Fake(send))))
            .unwrap()
            .name(BUS_NAME)
            .unwrap()
            .build()
            .await
            .unwrap();
        let client = zbus::connection::Builder::address(address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap();
        Self {
            _bus: bus,
            address,
            server,
            client,
            interactions,
        }
    }
    async fn incoming(&mut self) -> Interaction {
        tokio::time::timeout(Duration::from_secs(3), self.interactions.recv())
            .await
            .unwrap()
            .unwrap()
    }
}
async fn proxy(client: &zbus::Connection) -> zbus::Proxy<'_> {
    zbus::Proxy::new(
        client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.Usb",
    )
    .await
    .unwrap()
}
fn handle(name: &str) -> OwnedObjectPath {
    format!("/org/freedesktop/portal/desktop/request/usb/{name}")
        .try_into()
        .unwrap()
}
fn devices() -> Vec<(String, Options, Options)> {
    vec![
        (
            "first".into(),
            Options::new(),
            Options::from([("writable".into(), true.into())]),
        ),
        ("second".into(), Options::new(), Options::new()),
    ]
}
async fn acquire(client: zbus::Connection, name: String) -> zbus::Result<Response> {
    proxy(&client)
        .await
        .call(
            "AcquireDevices",
            &(
                handle(&name),
                "wayland:parent",
                "example.App",
                devices(),
                Options::new(),
            ),
        )
        .await
}
fn grant(devices: &[(&str, bool)]) -> Grant {
    Grant {
        devices: devices
            .iter()
            .map(|(id, writable)| SelectedDevice {
                id: (*id).into(),
                writable: *writable,
            })
            .collect(),
    }
}

#[tokio::test]
async fn exact_wire_and_result_validation_keep_usb_authority_within_the_offered_set() {
    let mut fixture = Fixture::new().await;
    let client = fixture.client.clone();
    let interface = proxy(&client).await;
    assert_eq!(interface.get_property::<u32>("version").await.unwrap(), 1);
    let introspection = zbus::Proxy::new(
        &fixture.client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.DBus.Introspectable",
    )
    .await
    .unwrap();
    let xml: String = introspection.call("Introspect", &()).await.unwrap();
    let method = xml
        .split("<method name=\"AcquireDevices\">")
        .nth(1)
        .unwrap()
        .split("</method>")
        .next()
        .unwrap();
    assert!(method.contains("type=\"a(sa{sv}a{sv})\""));
    assert!(
        method.find("name=\"parent_window\"").unwrap() < method.find("name=\"app_id\"").unwrap()
    );
    let outputs: Vec<_> = method
        .lines()
        .filter(|line| line.contains("direction=\"out\""))
        .collect();
    assert_eq!(outputs.len(), 2);
    assert!(outputs[0].contains("type=\"u\"") && outputs[1].contains("type=\"a{sv}\""));

    let task = tokio::spawn(acquire(fixture.client.clone(), "subset".into()));
    let pending = fixture.incoming().await;
    assert_eq!(pending.request.app_id, "example.App");
    assert_eq!(pending.request.parent_window, "wayland:parent");
    assert!(pending.request.devices[0].writable);
    assert!(!pending.request.devices[1].writable);
    pending
        .reply
        .send(Ok(Some(grant(&[("first", false)]))))
        .unwrap();
    let (code, mut values) = task.await.unwrap().unwrap();
    assert_eq!(code, 0);
    assert_eq!(values.len(), 1);
    let chosen: Vec<(String, Options)> = values.remove("devices").unwrap().try_into().unwrap();
    assert_eq!(chosen.len(), 1);
    assert_eq!(chosen[0].0, "first");
    assert!(!bool::try_from(chosen[0].1["writable"].try_clone().unwrap()).unwrap());

    for (name, selected) in [
        ("unknown", grant(&[("not-offered", false)])),
        ("escalated", grant(&[("second", true)])),
        ("repeated", grant(&[("first", false), ("first", true)])),
    ] {
        let task = tokio::spawn(acquire(fixture.client.clone(), name.into()));
        fixture
            .incoming()
            .await
            .reply
            .send(Ok(Some(selected)))
            .unwrap();
        let (code, results) = task.await.unwrap().unwrap();
        assert_eq!(code, 2);
        assert!(results.is_empty());
    }
    let task = tokio::spawn(acquire(fixture.client.clone(), "denied".into()));
    fixture.incoming().await.reply.send(Ok(None)).unwrap();
    let (code, results) = task.await.unwrap().unwrap();
    assert_eq!(code, 1);
    assert!(results.is_empty());

    let mut malformed = devices();
    malformed[0].2.insert(
        "writable".into(),
        OwnedValue::try_from(Value::from("yes")).unwrap(),
    );
    assert!(
        interface
            .call::<_, _, Response>(
                "AcquireDevices",
                &(
                    handle("malformed"),
                    "",
                    "example.App",
                    malformed,
                    Options::new()
                )
            )
            .await
            .is_err()
    );
    fixture.server.close().await.unwrap();
}

#[tokio::test]
async fn cancellation_disconnect_and_admission_cannot_leave_or_revive_a_usb_grant() {
    let mut fixture = Fixture::new().await;
    let stranger = zbus::connection::Builder::address(fixture.address.as_str())
        .unwrap()
        .build()
        .await
        .unwrap();
    let task = tokio::spawn(acquire(fixture.client.clone(), "cancel".into()));
    let pending = fixture.incoming().await;
    let path = handle("cancel");
    let foreign = zbus::Proxy::new(
        &stranger,
        BUS_NAME,
        path.clone(),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap();
    assert!(foreign.call::<_, _, ()>("Close", &()).await.is_err());
    let own = zbus::Proxy::new(
        &fixture.client,
        BUS_NAME,
        path,
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap();
    own.call::<_, _, ()>("Close", &()).await.unwrap();
    let (code, results) = tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(code, 1);
    assert!(results.is_empty());
    assert!(pending.reply.is_closed());
    assert!(own.call::<_, _, ()>("Close", &()).await.is_err());
    assert!(
        pending
            .reply
            .send(Ok(Some(grant(&[("first", true)]))))
            .is_err()
    );

    let lost = zbus::connection::Builder::address(fixture.address.as_str())
        .unwrap()
        .build()
        .await
        .unwrap();
    let task = tokio::spawn(acquire(lost.clone(), "lost".into()));
    let mut pending = fixture.incoming().await;
    lost.close().await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), pending.reply.closed())
        .await
        .unwrap();
    let _ = task.await;

    let mut tasks = Vec::new();
    let mut pending = Vec::new();
    for index in 0..16 {
        tasks.push(tokio::spawn(acquire(
            fixture.client.clone(),
            format!("slot{index}"),
        )));
        pending.push(fixture.incoming().await);
    }
    assert!(
        acquire(fixture.client.clone(), "overflow".into())
            .await
            .is_err()
    );
    for (index, task) in tasks.into_iter().enumerate() {
        let request = zbus::Proxy::new(
            &fixture.client,
            BUS_NAME,
            handle(&format!("slot{index}")),
            "org.freedesktop.impl.portal.Request",
        )
        .await
        .unwrap();
        request.call::<_, _, ()>("Close", &()).await.unwrap();
        assert_eq!(task.await.unwrap().unwrap().0, 1);
    }
    assert!(
        pending
            .iter()
            .all(|interaction| interaction.reply.is_closed())
    );
    let task = tokio::spawn(acquire(fixture.client.clone(), "capacity_returned".into()));
    fixture.incoming().await.reply.send(Ok(None)).unwrap();
    assert_eq!(task.await.unwrap().unwrap().0, 1);
    fixture.server.close().await.unwrap();
}
