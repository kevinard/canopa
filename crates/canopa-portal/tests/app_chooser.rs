use canopa_portal::{
    BUS_NAME, OBJECT_PATH,
    app_chooser::{AppChooser, AppRequest, AppSelection, Presenter},
    requests::{Options, Requests, Response},
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

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}
struct Interaction {
    request: AppRequest,
    updates: watch::Receiver<Vec<String>>,
    reply: oneshot::Sender<Option<AppSelection>>,
}
struct Fake(mpsc::UnboundedSender<Interaction>);
impl Presenter for Fake {
    fn present(
        &self,
        request: AppRequest,
        updates: watch::Receiver<Vec<String>>,
        _cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<AppSelection>>> {
        Box::pin(async move {
            let (reply, result) = oneshot::channel();
            self.0
                .send(Interaction {
                    request,
                    updates,
                    reply,
                })
                .unwrap();
            // Deliberately non-cooperative: the shared host must still retire it.
            Ok(result.await?)
        })
    }
}
fn value(value: impl Into<Value<'static>>) -> OwnedValue {
    OwnedValue::try_from(value.into()).unwrap()
}
fn handle(name: &str) -> OwnedObjectPath {
    format!("/org/freedesktop/portal/desktop/request/apps/{name}")
        .try_into()
        .unwrap()
}
async fn proxy(connection: &zbus::Connection) -> zbus::Proxy<'_> {
    zbus::Proxy::new(
        connection,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.AppChooser",
    )
    .await
    .unwrap()
}
async fn choose(
    connection: zbus::Connection,
    name: &'static str,
    applications: Vec<&'static str>,
    options: Options,
) -> zbus::Result<Response> {
    proxy(&connection)
        .await
        .call(
            "ChooseApplication",
            &(
                handle(name),
                "org.example.Reader",
                "wayland:parent",
                applications,
                options,
            ),
        )
        .await
}
async fn timeout<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), future)
        .await
        .expect("app chooser timed out")
}

#[tokio::test]
async fn applications_update_under_their_request_owner_and_accept_only_current_choices() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("bus.conf");
    std::fs::write(&config, "<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><auth>EXTERNAL</auth><policy context=\"default\"><allow send_destination=\"*\"/><allow receive_sender=\"*\"/><allow own=\"*\"/></policy></busconfig>").unwrap();
    let mut bus = Process(
        Command::new("dbus-daemon")
            .args(["--nofork", "--print-address=1", "--config-file"])
            .arg(&config)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut address = String::new();
    BufReader::new(bus.0.stdout.take().unwrap())
        .read_line(&mut address)
        .unwrap();
    let address = address.trim();
    let (sender, mut incoming) = mpsc::unbounded_channel();
    let server = zbus::connection::Builder::address(address)
        .unwrap()
        .serve_at(
            OBJECT_PATH,
            AppChooser::with_requests(Arc::new(Fake(sender)), Requests::default()),
        )
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
    let options = Options::from([
        ("last_choice".into(), value("a.desktop")),
        ("content_type".into(), value("text/plain")),
        ("filename".into(), value("notes.txt")),
        ("uri".into(), value("file:///tmp/notes.txt")),
        ("modal".into(), false.into()),
        ("activation_token".into(), value("incoming-token")),
    ]);
    let call = tokio::spawn(choose(
        client.clone(),
        "live",
        vec!["a.desktop", "b.desktop", "a.desktop"],
        options,
    ));
    let mut interaction = timeout(incoming.recv()).await.unwrap();
    assert_eq!(interaction.request.choices, ["a.desktop", "b.desktop"]);
    assert_eq!(
        interaction.request.last_choice.as_deref(),
        Some("a.desktop")
    );
    assert_eq!(
        interaction.request.content_type.as_deref(),
        Some("text/plain")
    );
    assert_eq!(interaction.request.filename.as_deref(), Some("notes.txt"));
    assert_eq!(
        interaction.request.activation_token.as_deref(),
        Some("incoming-token")
    );
    assert!(!interaction.request.modal);
    assert!(
        matches!(proxy(&stranger).await.call::<_, _, ()>("UpdateChoices", &(handle("live"), vec!["c.desktop"])).await, Err(zbus::Error::MethodError(ref name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.AccessDenied")
    );
    timeout(proxy(&client).await.call::<_, _, ()>(
        "UpdateChoices",
        &(handle("live"), vec!["c.desktop", "a.desktop"]),
    ))
    .await
    .unwrap();
    timeout(interaction.updates.changed()).await.unwrap();
    assert_eq!(*interaction.updates.borrow(), ["c.desktop", "a.desktop"]);
    interaction
        .reply
        .send(Some(AppSelection {
            choice: "c.desktop".into(),
            activation_token: Some("fresh-token".into()),
        }))
        .unwrap();
    let (status, results) = timeout(call).await.unwrap().unwrap();
    assert_eq!(status, 0);
    assert_eq!(
        String::try_from(results["choice"].try_clone().unwrap()).unwrap(),
        "c.desktop"
    );
    assert_eq!(
        String::try_from(results["activation_token"].try_clone().unwrap()).unwrap(),
        "fresh-token"
    );
    assert!(
        proxy(&client)
            .await
            .call::<_, _, ()>("UpdateChoices", &(handle("live"), vec!["a.desktop"]))
            .await
            .is_err()
    );

    let removed = tokio::spawn(choose(
        client.clone(),
        "removed",
        vec!["a.desktop", "b.desktop"],
        Options::new(),
    ));
    let interaction = timeout(incoming.recv()).await.unwrap();
    proxy(&client)
        .await
        .call::<_, _, ()>("UpdateChoices", &(handle("removed"), vec!["b.desktop"]))
        .await
        .unwrap();
    interaction
        .reply
        .send(Some(AppSelection {
            choice: "a.desktop".into(),
            activation_token: None,
        }))
        .unwrap();
    let (status, results) = timeout(removed).await.unwrap().unwrap();
    assert_eq!(status, 2);
    assert!(results.is_empty());

    let stale = tokio::spawn(choose(
        client.clone(),
        "token",
        vec!["a.desktop"],
        Options::from([("activation_token".into(), value("old-token"))]),
    ));
    timeout(incoming.recv())
        .await
        .unwrap()
        .reply
        .send(Some(AppSelection {
            choice: "a.desktop".into(),
            activation_token: Some("old-token".into()),
        }))
        .unwrap();
    let (status, results) = timeout(stale).await.unwrap().unwrap();
    assert_eq!(status, 0);
    assert!(!results.contains_key("activation_token"));

    let pending = tokio::spawn(choose(
        client.clone(),
        "cancel",
        vec!["a.desktop"],
        Options::new(),
    ));
    let interaction = timeout(incoming.recv()).await.unwrap();
    let request = zbus::Proxy::new(
        &client,
        BUS_NAME,
        handle("cancel"),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap();
    timeout(request.call::<_, _, ()>("Close", &()))
        .await
        .unwrap();
    assert_eq!(timeout(pending).await.unwrap().unwrap().0, 1);
    assert!(interaction.reply.send(None).is_err());
    assert!(
        proxy(&client)
            .await
            .call::<_, _, ()>("UpdateChoices", &(handle("cancel"), vec!["b.desktop"]))
            .await
            .is_err()
    );

    let no_choices = tokio::spawn(choose(client.clone(), "all", vec![], Options::new()));
    let interaction = timeout(incoming.recv()).await.unwrap();
    assert!(interaction.request.choices.is_empty());
    interaction
        .reply
        .send(Some(AppSelection {
            choice: "installed.desktop".into(),
            activation_token: None,
        }))
        .unwrap();
    assert_eq!(timeout(no_choices).await.unwrap().unwrap().0, 0);

    assert!(
        matches!(choose(client.clone(), "bad", vec![""], Options::new()).await, Err(zbus::Error::MethodError(ref name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.InvalidArgs")
    );
    assert!(
        matches!(choose(client, "bad", vec!["a.desktop"], Options::from([("modal".into(), 1_u32.into())])).await, Err(zbus::Error::MethodError(ref name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.InvalidArgs")
    );
    assert!(incoming.try_recv().is_err());
    server.close().await.unwrap();
}
