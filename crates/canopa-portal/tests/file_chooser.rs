use canopa_portal::{
    BUS_NAME, OBJECT_PATH, Settings,
    file_chooser::{ChooserRequest, FileChooser, Options, Presenter, Response, Selection},
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
    request: ChooserRequest,
    reply: oneshot::Sender<Option<Selection>>,
    ended: oneshot::Receiver<()>,
}
struct Fake(mpsc::UnboundedSender<Interaction>);
struct End(Option<oneshot::Sender<()>>);
impl Drop for End {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            sender.send(()).ok();
        }
    }
}
impl Presenter for Fake {
    fn present(
        &self,
        request: ChooserRequest,
        mut cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<Selection>>> {
        Box::pin(async move {
            let (reply, result) = oneshot::channel();
            let (end, ended) = oneshot::channel();
            let _end = End(Some(end));
            self.0
                .send(Interaction {
                    request,
                    reply,
                    ended,
                })
                .unwrap();
            let selection = tokio::select! {
                biased;
                _ = cancelled.changed() => None,
                result = result => result?,
            };
            Ok(selection)
        })
    }
}
fn value(value: impl Into<Value<'static>>) -> OwnedValue {
    OwnedValue::try_from(value.into()).unwrap()
}
fn handle(name: &str) -> OwnedObjectPath {
    format!("/org/freedesktop/portal/desktop/request/test/{name}")
        .try_into()
        .unwrap()
}
async fn proxy(connection: &zbus::Connection) -> zbus::Proxy<'_> {
    zbus::Proxy::new(
        connection,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.FileChooser",
    )
    .await
    .unwrap()
}
fn selection(paths: Vec<Vec<u8>>) -> Selection {
    Selection {
        paths,
        choices: vec![],
        current_filter: None,
        writable: true,
    }
}
async fn call(
    connection: zbus::Connection,
    method: &'static str,
    handle: OwnedObjectPath,
    options: Options,
) -> zbus::Result<Response> {
    proxy(&connection)
        .await
        .call(
            method,
            &(
                handle,
                "org.example.Editor",
                "wayland:exported-parent",
                "Choose files",
                options,
            ),
        )
        .await
}
async fn timeout<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), future)
        .await
        .expect("portal interaction timed out")
}

#[tokio::test]
async fn file_chooser_wire_and_request_lifetimes() {
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
        .serve_at(OBJECT_PATH, FileChooser::new(Arc::new(Fake(sender))))
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
    let filters = vec![(
        "Text".to_owned(),
        vec![(0_u32, "*.txt".to_owned()), (1, "text/plain".into())],
    )];
    let choices = vec![
        (
            "encoding",
            "Encoding",
            vec![("utf8", "UTF-8"), ("latin1", "Latin-1")],
            "latin1",
        ),
        ("compress", "Compress", vec![], "true"),
    ];
    let options = Options::from([
        ("multiple".into(), true.into()),
        ("filters".into(), value(filters.clone())),
        ("choices".into(), value(choices)),
        ("current_folder".into(), value(b"/tmp/\0".to_vec())),
        ("future-option".into(), true.into()),
    ]);
    let open = tokio::spawn(call(client.clone(), "OpenFile", handle("open"), options));
    let interaction = timeout(incoming.recv()).await.unwrap();
    assert_eq!(interaction.request.title, "Choose files");
    assert_eq!(interaction.request.app_id, "org.example.Editor");
    assert_eq!(interaction.request.parent_window, "wayland:exported-parent");
    assert!(interaction.request.multiple && interaction.request.modal);
    assert_eq!(interaction.request.current_filter, Some(filters[0].clone()));
    interaction
        .reply
        .send(Some(Selection {
            paths: vec![
                b"/tmp/with space.txt".to_vec(),
                b"/tmp/raw-\xff.txt".to_vec(),
            ],
            choices: vec![
                ("encoding".into(), "utf8".into()),
                ("compress".into(), "false".into()),
            ],
            current_filter: Some(filters[0].clone()),
            writable: false,
        }))
        .unwrap();
    let (response, results) = timeout(open).await.unwrap().unwrap();
    assert_eq!(response, 0);
    assert_eq!(
        Vec::<String>::try_from(results["uris"].try_clone().unwrap()).unwrap(),
        ["file:///tmp/with%20space.txt", "file:///tmp/raw-%FF.txt"]
    );
    assert!(!bool::try_from(&results["writable"]).unwrap());
    assert_eq!(
        Vec::<(String, String)>::try_from(results["choices"].try_clone().unwrap()).unwrap(),
        [
            ("encoding".into(), "utf8".into()),
            ("compress".into(), "false".into())
        ]
    );
    assert_eq!(
        <(String, Vec<(u32, String)>)>::try_from(results["current_filter"].try_clone().unwrap())
            .unwrap(),
        filters[0]
    );
    let removed = zbus::Proxy::new(
        &client,
        BUS_NAME,
        handle("open"),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap();
    assert!(removed.call::<_, _, ()>("Close", &()).await.is_err());

    // Calls are independent and Settings remains responsive while dialogs wait.
    let first = tokio::spawn(call(
        client.clone(),
        "OpenFile",
        handle("cancel"),
        Options::new(),
    ));
    let waiting = timeout(incoming.recv()).await.unwrap();
    let second = tokio::spawn(call(
        client.clone(),
        "SaveFile",
        handle("save"),
        Options::new(),
    ));
    let save = timeout(incoming.recv()).await.unwrap();
    let settings = zbus::Proxy::new(
        &client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.Settings",
    )
    .await
    .unwrap();
    assert_eq!(
        timeout(settings.get_property::<u32>("version"))
            .await
            .unwrap(),
        1
    );
    let duplicate = timeout(call(
        client.clone(),
        "OpenFile",
        handle("cancel"),
        Options::new(),
    ))
    .await;
    assert!(
        matches!(duplicate, Err(zbus::Error::MethodError(ref name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.InvalidArgs")
    );
    let wrong_owner = zbus::Proxy::new(
        &stranger,
        BUS_NAME,
        handle("cancel"),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap();
    assert!(
        matches!(wrong_owner.call::<_, _, ()>("Close", &()).await, Err(zbus::Error::MethodError(ref name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.AccessDenied")
    );
    let own = zbus::Proxy::new(
        &client,
        BUS_NAME,
        handle("cancel"),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap();
    timeout(own.call::<_, _, ()>("Close", &())).await.unwrap();
    timeout(waiting.ended).await.unwrap();
    assert_eq!(timeout(first).await.unwrap().unwrap().0, 1);
    save.reply
        .send(Some(selection(vec![b"/tmp/new.txt".to_vec()])))
        .unwrap();
    let response = timeout(second).await.unwrap().unwrap();
    assert_eq!(response.0, 0);
    assert!(!response.1.contains_key("writable"));

    let options = Options::from([(
        "files".into(),
        value(vec![b"one\0".to_vec(), b"raw-\xff\0".to_vec()]),
    )]);
    let many = tokio::spawn(call(client.clone(), "SaveFiles", handle("many"), options));
    let interaction = timeout(incoming.recv()).await.unwrap();
    assert!(interaction.request.directory);
    interaction
        .reply
        .send(Some(selection(vec![
            b"/tmp/one".to_vec(),
            b"/tmp/raw-\xff".to_vec(),
        ])))
        .unwrap();
    let response = timeout(many).await.unwrap().unwrap();
    assert_eq!(response.0, 0);
    assert_eq!(
        Vec::<String>::try_from(response.1["uris"].try_clone().unwrap()).unwrap(),
        ["file:///tmp/one", "file:///tmp/raw-%FF"]
    );

    for (method, options) in [
        (
            "OpenFile",
            Options::from([("multiple".into(), 1_u32.into())]),
        ),
        (
            "SaveFile",
            Options::from([("current_folder".into(), value(b"/not-terminated".to_vec()))]),
        ),
        (
            "SaveFiles",
            Options::from([("files".into(), value(vec![b"../escape\0".to_vec()]))]),
        ),
        ("SaveFiles", Options::new()),
    ] {
        assert!(
            matches!(timeout(call(client.clone(), method, handle("invalid"), options)).await, Err(zbus::Error::MethodError(ref name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.InvalidArgs")
        );
        assert!(incoming.try_recv().is_err());
    }
    // Renderer failures and invalid successes never return grants.
    let bad = tokio::spawn(call(
        client.clone(),
        "OpenFile",
        handle("bad"),
        Options::new(),
    ));
    timeout(incoming.recv())
        .await
        .unwrap()
        .reply
        .send(Some(selection(vec![b"relative".to_vec()])))
        .unwrap();
    let response = timeout(bad).await.unwrap().unwrap();
    assert_eq!(response.0, 2);
    assert!(response.1.is_empty());
    let dismissed = tokio::spawn(call(
        client.clone(),
        "OpenFile",
        handle("dismiss"),
        Options::new(),
    ));
    timeout(incoming.recv())
        .await
        .unwrap()
        .reply
        .send(None)
        .unwrap();
    assert_eq!(timeout(dismissed).await.unwrap().unwrap().0, 1);

    let failed = tokio::spawn(call(
        client.clone(),
        "OpenFile",
        handle("failed"),
        Options::new(),
    ));
    drop(timeout(incoming.recv()).await.unwrap());
    let response = timeout(failed).await.unwrap().unwrap();
    assert_eq!(response.0, 2);
    assert!(response.1.is_empty());

    let transient = zbus::connection::Builder::address(address)
        .unwrap()
        .build()
        .await
        .unwrap();
    let gone = tokio::spawn(call(
        transient.clone(),
        "OpenFile",
        handle("gone"),
        Options::new(),
    ));
    let waiting = timeout(incoming.recv()).await.unwrap();
    transient.close().await.unwrap();
    timeout(gone).await.unwrap().unwrap_err();
    // Caller loss drops presentation and unexports the request even without a UI reply.
    let check = zbus::Proxy::new(
        &client,
        BUS_NAME,
        handle("gone"),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap();
    timeout(async {
        loop {
            if matches!(check.call::<_, _, ()>("Close", &()).await,
                Err(zbus::Error::MethodError(ref name, _, _)) if matches!(name.as_str(), "org.freedesktop.DBus.Error.UnknownObject" | "org.freedesktop.DBus.Error.UnknownMethod")) { break; }
            tokio::task::yield_now().await;
        }
    })
    .await;
    drop(waiting);
    server.close().await.unwrap();
}

#[tokio::test]
async fn remembered_folders_respect_hints_and_only_valid_completed_choices_update_them() {
    use canopa_portal::{file_chooser::history::History, requests::Requests};
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let history = History::memory();
    history.save("org.example.Editor".into(), a.clone()).await;
    let config = root.path().join("bus.conf");
    std::fs::write(&config, "<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><auth>EXTERNAL</auth><policy context=\"default\"><allow send_destination=\"*\"/><allow receive_sender=\"*\"/><allow own=\"*\"/></policy></busconfig>").unwrap();
    let mut bus = Process(
        Command::new("dbus-daemon")
            .args(["--nofork", "--print-address=1", "--config-file"])
            .arg(config)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut address = String::new();
    BufReader::new(bus.0.stdout.take().unwrap())
        .read_line(&mut address)
        .unwrap();
    let (sender, mut incoming) = mpsc::unbounded_channel();
    let server = zbus::connection::Builder::address(address.trim())
        .unwrap()
        .serve_at(
            OBJECT_PATH,
            FileChooser::with_history(Arc::new(Fake(sender)), Requests::default(), history.clone()),
        )
        .unwrap()
        .name(BUS_NAME)
        .unwrap()
        .build()
        .await
        .unwrap();
    let client = zbus::connection::Builder::address(address.trim())
        .unwrap()
        .build()
        .await
        .unwrap();
    fn bytes(path: &std::path::Path) -> Vec<u8> {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    }
    fn hint(path: &std::path::Path) -> Vec<u8> {
        let mut value = bytes(path);
        value.push(0);
        value
    }

    let accepted = tokio::spawn(call(
        client.clone(),
        "SaveFile",
        handle("remember"),
        Options::new(),
    ));
    let interaction = timeout(incoming.recv()).await.unwrap();
    assert_eq!(interaction.request.current_folder, Some(hint(&a)));
    assert!(
        interaction.request.current_file.is_none(),
        "Remember only a folder, never a selected file"
    );
    interaction
        .reply
        .send(Some(selection(vec![bytes(&b.join("new.txt"))])))
        .unwrap();
    assert_eq!(timeout(accepted).await.unwrap().unwrap().0, 0);
    assert_eq!(
        history.load("org.example.Editor".into()).await,
        Some(b.clone())
    );
    assert!(!b.join("new.txt").exists());

    for (name, options) in [
        (
            "folder_hint",
            Options::from([("current_folder".into(), value(hint(&a)))]),
        ),
        (
            "file_hint",
            Options::from([("current_file".into(), value(hint(&a.join("specific.txt"))))]),
        ),
    ] {
        let selected = tokio::spawn(call(client.clone(), "SaveFile", handle(name), options));
        let interaction = timeout(incoming.recv()).await.unwrap();
        if name == "folder_hint" {
            assert_eq!(interaction.request.current_folder, Some(hint(&a)));
        } else {
            assert_eq!(
                interaction.request.current_file,
                Some(hint(&a.join("specific.txt")))
            );
            assert!(interaction.request.current_folder.is_none());
        }
        interaction.reply.send(None).unwrap();
        assert_eq!(timeout(selected).await.unwrap().unwrap().0, 1);
        assert_eq!(
            history.load("org.example.Editor".into()).await,
            Some(b.clone())
        );
    }

    let invalid = tokio::spawn(call(
        client.clone(),
        "SaveFile",
        handle("invalid_choice"),
        Options::new(),
    ));
    timeout(incoming.recv())
        .await
        .unwrap()
        .reply
        .send(Some(selection(vec![b"relative".to_vec()])))
        .unwrap();
    assert_eq!(timeout(invalid).await.unwrap().unwrap().0, 2);
    assert_eq!(
        history.load("org.example.Editor".into()).await,
        Some(b.clone())
    );

    let cancelled = tokio::spawn(call(
        client.clone(),
        "SaveFile",
        handle("closed_choice"),
        Options::new(),
    ));
    let waiting = timeout(incoming.recv()).await.unwrap();
    let owned = zbus::Proxy::new(
        &client,
        BUS_NAME,
        handle("closed_choice"),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap();
    owned.call::<_, _, ()>("Close", &()).await.unwrap();
    timeout(waiting.ended).await.unwrap();
    assert_eq!(timeout(cancelled).await.unwrap().unwrap().0, 1);
    assert_eq!(history.load("org.example.Editor".into()).await, Some(b));
    server.close().await.unwrap();
}
