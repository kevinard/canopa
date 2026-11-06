use canopa_portal::{
    BUS_NAME, OBJECT_PATH, Settings,
    requests::{Options, Requests},
    screenshot::{Kind, Outcome, Presenter, Screenshot, ScreenshotRequest},
};
use futures_util::future::BoxFuture;
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, watch};
use zbus::{
    Connection,
    zvariant::{OwnedObjectPath, Value},
};

struct Bus {
    process: Child,
    address: String,
}
impl Bus {
    fn new() -> Self {
        let mut process = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut address = String::new();
        BufReader::new(process.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        Self {
            process,
            address: address.trim().into(),
        }
    }
    async fn connect(&self) -> Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap()
    }
}
impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}
struct Interaction {
    request: ScreenshotRequest,
    cancelled: watch::Receiver<bool>,
    reply: oneshot::Sender<anyhow::Result<Option<Outcome>>>,
}
struct Fake(mpsc::UnboundedSender<Interaction>);
struct Windows(Fake);
impl Presenter for Windows {
    fn available_targets<'a>(&'a self, _: &'a Connection) -> BoxFuture<'a, u32> {
        Box::pin(async { 15 })
    }
    fn present<'a>(
        &'a self,
        request: ScreenshotRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<Outcome>>> {
        self.0.present(request, cancelled)
    }
}
impl Presenter for Fake {
    fn present<'a>(
        &'a self,
        request: ScreenshotRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<Outcome>>> {
        Box::pin(async move {
            let (reply, response) = oneshot::channel();
            self.0
                .send(Interaction {
                    request,
                    cancelled,
                    reply,
                })
                .unwrap();
            response.await?
        })
    }
}
fn path(name: &str) -> OwnedObjectPath {
    format!("/org/freedesktop/portal/desktop/request/screenshot/{name}")
        .try_into()
        .unwrap()
}

#[tokio::test]
async fn window_targets_reach_the_presenter_without_becoming_screen_requests() {
    let bus = Bus::new();
    let server = bus.connect().await;
    let client = bus.connect().await;
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let screenshot =
        Screenshot::with_requests(Arc::new(Windows(Fake(sender))), Requests::default());
    server
        .object_server()
        .at(OBJECT_PATH, screenshot)
        .await
        .unwrap();
    server.request_name(BUS_NAME).await.unwrap();
    let proxy = zbus::Proxy::new(
        &client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.Screenshot",
    )
    .await
    .unwrap();
    assert_eq!(
        proxy.get_property::<u32>("AvailableTargets").await.unwrap(),
        15
    );
    for (target, kind) in [(2u32, Kind::Window), (8u32, Kind::ActiveWindow)] {
        let calling = tokio::spawn(call(
            client.clone(),
            "window",
            "Screenshot",
            Options::from([(
                "target".into(),
                Value::from(target).try_into_owned().unwrap(),
            )]),
        ));
        let interaction = next(&mut receiver).await;
        assert_eq!(interaction.request.kind, kind);
        assert!(!interaction.request.allow_area);
        assert!(
            interaction
                .reply
                .send(Ok(Some(Outcome::Image(
                    "file:///tmp/window-screenshot.png".into(),
                ))))
                .is_ok()
        );
        let (response, result) = calling.await.unwrap().unwrap();
        assert_eq!(response, 0);
        assert_eq!(
            <&str>::try_from(&result["uri"]).unwrap(),
            "file:///tmp/window-screenshot.png"
        );
    }
}
async fn call(
    client: Connection,
    name: &str,
    method: &str,
    options: Options,
) -> zbus::Result<(u32, Options)> {
    zbus::Proxy::new(
        &client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.Screenshot",
    )
    .await?
    .call(
        method,
        &(path(name), "org.canopa.Test", "wayland:parent", options),
    )
    .await
}
async fn next(receiver: &mut mpsc::UnboundedReceiver<Interaction>) -> Interaction {
    tokio::time::timeout(Duration::from_secs(3), receiver.recv())
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn exact_screenshot_contract_permissions_results_and_sender_lifetimes() {
    let bus = Bus::new();
    let server = bus.connect().await;
    let client = bus.connect().await;
    let foreign = bus.connect().await;
    let (sender, mut receiver) = mpsc::unbounded_channel();
    server
        .object_server()
        .at(
            OBJECT_PATH,
            Settings::new(canopa_theme::Appearance::default()),
        )
        .await
        .unwrap();
    server
        .object_server()
        .at(
            OBJECT_PATH,
            Screenshot::with_requests(Arc::new(Fake(sender)), Requests::default()),
        )
        .await
        .unwrap();
    server.request_name(BUS_NAME).await.unwrap();
    let proxy = zbus::Proxy::new(
        &client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.Screenshot",
    )
    .await
    .unwrap();
    assert_eq!(proxy.get_property::<u32>("version").await.unwrap(), 3);
    assert_eq!(
        proxy.get_property::<u32>("AvailableTargets").await.unwrap(),
        5
    );
    let options = Options::from([
        (
            "target".into(),
            Value::from(4_u32).try_into_owned().unwrap(),
        ),
        ("modal".into(), Value::from(false).try_into_owned().unwrap()),
    ]);
    let image = tokio::spawn(call(client.clone(), "image", "Screenshot", options));
    let interaction = next(&mut receiver).await;
    assert_eq!(interaction.request.kind, Kind::Area);
    assert!(!interaction.request.modal);
    assert_eq!(interaction.request.parent_window, "wayland:parent");
    assert_eq!(interaction.request.app_id, "org.canopa.Test");
    let settings = zbus::Proxy::new(
        &client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.Settings",
    )
    .await
    .unwrap();
    let values: std::collections::HashMap<String, Options> = tokio::time::timeout(
        Duration::from_secs(3),
        settings.call("ReadAll", &(vec!["org.freedesktop.appearance"],)),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(values.contains_key("org.freedesktop.appearance"));
    interaction
        .reply
        .send(Ok(Some(Outcome::Image("file:///tmp/reviewed.png".into()))))
        .ok()
        .unwrap();
    let (status, results) = image.await.unwrap().unwrap();
    assert_eq!(status, 0);
    assert_eq!(results["uri"].value_signature().to_string(), "s");

    let color = tokio::spawn(call(client.clone(), "color", "PickColor", Options::new()));
    let interaction = next(&mut receiver).await;
    assert_eq!(interaction.request.kind, Kind::Color);
    interaction
        .reply
        .send(Ok(Some(Outcome::Color([0.0, 0.5, 1.0]))))
        .ok()
        .unwrap();
    let (status, results) = color.await.unwrap().unwrap();
    assert_eq!(status, 0);
    assert_eq!(results["color"].value_signature().to_string(), "(ddd)");
    let invalid = tokio::spawn(call(client.clone(), "invalid", "PickColor", Options::new()));
    next(&mut receiver)
        .await
        .reply
        .send(Ok(Some(Outcome::Color([f64::NAN, 0.0, 0.0]))))
        .ok()
        .unwrap();
    let (status, results) = invalid.await.unwrap().unwrap();
    assert_eq!(status, 2);
    assert!(results.is_empty());

    let closing = tokio::spawn(call(client.clone(), "close", "Screenshot", Options::new()));
    let mut interaction = next(&mut receiver).await;
    let wrong = zbus::Proxy::new(
        &foreign,
        BUS_NAME,
        path("close"),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap();
    assert!(wrong.call::<_, _, ()>("Close", &()).await.is_err());
    let own = zbus::Proxy::new(
        &client,
        BUS_NAME,
        path("close"),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap();
    own.call::<_, _, ()>("Close", &()).await.unwrap();
    assert_eq!(closing.await.unwrap().unwrap().0, 1);
    tokio::time::timeout(Duration::from_secs(3), async {
        while !*interaction.cancelled.borrow() {
            interaction.cancelled.changed().await.unwrap();
        }
    })
    .await
    .unwrap();

    let loss_client = bus.connect().await;
    let loss = tokio::spawn(call(
        loss_client.clone(),
        "loss",
        "Screenshot",
        Options::new(),
    ));
    let mut interaction = next(&mut receiver).await;
    loss_client.close().await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while !*interaction.cancelled.borrow() {
            interaction.cancelled.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert!(loss.await.unwrap().is_err());
    assert!(
        call(
            client,
            "bad_target",
            "Screenshot",
            Options::from([(
                "target".into(),
                Value::from(8_u32).try_into_owned().unwrap()
            )])
        )
        .await
        .is_err()
    );
}
