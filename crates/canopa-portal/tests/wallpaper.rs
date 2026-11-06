use canopa_portal::{
    BUS_NAME, OBJECT_PATH, Settings,
    requests::Options,
    wallpaper::{Destination, Presenter, Wallpaper, WallpaperRequest},
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
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

struct Bus {
    child: Child,
    address: String,
}
impl Bus {
    fn new() -> Self {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let mut address = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        assert!(!address.trim().is_empty());
        Self {
            child,
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
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Interaction {
    request: WallpaperRequest,
    cancelled: watch::Receiver<bool>,
    reply: oneshot::Sender<anyhow::Result<bool>>,
}
struct Fake(mpsc::UnboundedSender<Interaction>);
impl Presenter for Fake {
    fn present<'a>(
        &'a self,
        request: WallpaperRequest,
        _: &'a Connection,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<bool>> {
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

fn handle(name: &str) -> OwnedObjectPath {
    format!("/org/freedesktop/portal/desktop/request/wallpaper/{name}")
        .try_into()
        .unwrap()
}
fn value(v: impl Into<Value<'static>>) -> OwnedValue {
    v.into().try_into_owned().unwrap()
}
async fn call(connection: Connection, name: &str, options: Options) -> zbus::Result<u32> {
    zbus::Proxy::new(
        &connection,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.Wallpaper",
    )
    .await?
    .call(
        "SetWallpaperURI",
        &(
            handle(name),
            "org.canopa.Fixture",
            "wayland:parent",
            "file:///fixture.png",
            options,
        ),
    )
    .await
}

#[tokio::test]
async fn wallpaper_wire_cancellation_identity_and_responses_match_the_single_result_contract() {
    let bus = Bus::new();
    let (sender, mut incoming) = mpsc::unbounded_channel();
    let server = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .name(BUS_NAME)
        .unwrap()
        .serve_at(OBJECT_PATH, Wallpaper::new(Arc::new(Fake(sender))))
        .unwrap()
        .serve_at(
            OBJECT_PATH,
            Settings::new(canopa_theme::Appearance::default()),
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    let client = bus.connect().await;
    let other = bus.connect().await;
    let xml: String = client
        .call_method(
            Some(BUS_NAME),
            OBJECT_PATH,
            Some("org.freedesktop.DBus.Introspectable"),
            "Introspect",
            &(),
        )
        .await
        .unwrap()
        .body()
        .deserialize()
        .unwrap();
    let wallpaper = xml
        .split("<interface name=\"org.freedesktop.impl.portal.Wallpaper\">")
        .nth(1)
        .unwrap()
        .split("</interface>")
        .next()
        .unwrap();
    assert!(wallpaper.contains("name=\"response\" type=\"u\" direction=\"out\""));
    assert!(wallpaper.contains("method name=\"SetWallpaperURI\""));
    assert!(!wallpaper.contains("<property"));
    assert!(!wallpaper.contains("name=\"results\""));
    for (token, outcome, code) in [
        ("accepted", Ok(true), 0),
        ("cancelled", Ok(false), 1),
        ("failed", Err(anyhow::anyhow!("private failure")), 2),
    ] {
        let task = tokio::spawn({
            let client = client.clone();
            async move {
                call(
                    client,
                    token,
                    Options::from([("set-on".into(), value("both"))]),
                )
                .await
            }
        });
        let interaction = tokio::time::timeout(Duration::from_secs(3), incoming.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(interaction.request.destination, Destination::Both);
        assert_eq!(interaction.request.app_id, "org.canopa.Fixture");
        assert_eq!(interaction.request.parent_window, "wayland:parent");
        let settings = zbus::Proxy::new(
            &other,
            BUS_NAME,
            OBJECT_PATH,
            "org.freedesktop.impl.portal.Settings",
        )
        .await
        .unwrap();
        let _: Options = settings
            .call::<_, _, std::collections::HashMap<String, Options>>(
                "ReadAll",
                &(Vec::<String>::new(),),
            )
            .await
            .unwrap()
            .remove("org.freedesktop.appearance")
            .unwrap();
        interaction.reply.send(outcome).unwrap();
        assert_eq!(task.await.unwrap().unwrap(), code);
    }
    let task = tokio::spawn({
        let client = client.clone();
        async move { call(client, "close", Options::new()).await }
    });
    let mut interaction = tokio::time::timeout(Duration::from_secs(3), incoming.recv())
        .await
        .unwrap()
        .unwrap();
    let close = |connection: Connection| async move {
        zbus::Proxy::new(
            &connection,
            BUS_NAME,
            handle("close"),
            "org.freedesktop.impl.portal.Request",
        )
        .await
        .unwrap()
        .call::<_, _, ()>("Close", &())
        .await
    };
    assert!(
        close(other.clone())
            .await
            .unwrap_err()
            .to_string()
            .contains("AccessDenied")
    );
    close(client.clone()).await.unwrap();
    assert_eq!(task.await.unwrap().unwrap(), 1);
    if !*interaction.cancelled.borrow() {
        interaction.cancelled.changed().await.unwrap();
    }
    assert!(*interaction.cancelled.borrow());
    assert!(interaction.reply.send(Ok(true)).is_err());

    let lost = bus.connect().await;
    let task = tokio::spawn({
        let client = lost.clone();
        async move { call(client, "lost", Options::new()).await }
    });
    let mut interaction = tokio::time::timeout(Duration::from_secs(3), incoming.recv())
        .await
        .unwrap()
        .unwrap();
    lost.close().await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), interaction.cancelled.changed())
        .await
        .unwrap()
        .unwrap();
    assert!(*interaction.cancelled.borrow());
    assert!(task.await.unwrap().is_err());
    assert!(interaction.reply.send(Ok(true)).is_err());
    drop(server);
}

#[cfg(feature = "desktop")]
mod production {
    use super::*;
    use std::{
        fs::{self, File},
        io::Read,
        path::Path,
        sync::Mutex,
    };

    struct Owner {
        applied: Arc<Mutex<Vec<Vec<u8>>>>,
    }
    #[zbus::interface(name = "org.canopa.Shell.Wallpaper1")]
    impl Owner {
        #[zbus(property)]
        fn destinations(&self) -> u32 {
            3
        }
        fn apply(&self, image: zbus::zvariant::OwnedFd, destinations: u32) {
            assert_eq!(destinations, 3);
            let mut file = File::from(std::os::fd::OwnedFd::from(image));
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).unwrap();
            self.applied.lock().unwrap().push(bytes);
        }
    }

    async fn server(bus: &Bus, source: &Path, helper: &Path) -> Connection {
        let process = Arc::new(canopa_portal::process::DialogProcess {
            executable: "/usr/bin/python3".into(),
            arguments: vec![helper.as_os_str().into(), source.as_os_str().into()],
        });
        zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name(BUS_NAME)
            .unwrap()
            .serve_at(
                OBJECT_PATH,
                Wallpaper::new(Arc::new(canopa_portal::wallpaper::Production::new(process))),
            )
            .unwrap()
            .build()
            .await
            .unwrap()
    }

    async fn apply(client: &Connection, source: &Path) -> u32 {
        zbus::Proxy::new(
            client,
            BUS_NAME,
            OBJECT_PATH,
            "org.freedesktop.impl.portal.Wallpaper",
        )
        .await
        .unwrap()
        .call(
            "SetWallpaperURI",
            &(
                handle("production"),
                "app",
                "",
                url::Url::from_file_path(source).unwrap().as_str(),
                Options::from([("set-on".into(), value("both"))]),
            ),
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn accepted_review_installs_the_frozen_image_after_the_caller_source_is_deleted() {
        let bus = Bus::new();
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.png");
        image::RgbaImage::from_pixel(33, 19, image::Rgba([73, 19, 201, 255]))
            .save(&source)
            .unwrap();
        let helper = directory.path().join("review.py");
        fs::write(&helper, "import json,sys\nfrom pathlib import Path\nrequest=json.load(sys.stdin)\nassert request['destination']=='Both'\nassert request['width']==33 and request['height']==19\nassert 'uri' not in request\nPath(sys.argv[1]).unlink()\nprint('true')\n").unwrap();
        let applied = Arc::new(Mutex::new(Vec::new()));
        let _owner = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name("org.canopa.Shell.Wallpaper")
            .unwrap()
            .serve_at(
                "/org/canopa/Shell/Wallpaper",
                Owner {
                    applied: applied.clone(),
                },
            )
            .unwrap()
            .build()
            .await
            .unwrap();
        let _backend = server(&bus, &source, &helper).await;
        let client = bus.connect().await;
        assert_eq!(apply(&client, &source).await, 0);
        assert!(!source.exists());
        let stored = applied.lock().unwrap();
        assert_eq!(stored.len(), 1);
        let actual = image::load_from_memory(&stored[0]).unwrap().into_rgba8();
        assert_eq!(actual.dimensions(), (33, 19));
        assert_eq!(actual.get_pixel(10, 10).0, [73, 19, 201, 255]);
    }

    #[tokio::test]
    async fn absent_owner_fails_before_starting_a_review_or_opening_the_source() {
        let bus = Bus::new();
        let directory = tempfile::tempdir().unwrap();
        let helper = directory.path().join("must-not-run.py");
        let source = directory.path().join("not-an-image");
        let _backend = server(&bus, &source, &helper).await;
        assert_eq!(apply(&bus.connect().await, &source).await, 2);
        assert!(!helper.exists());
    }

    #[tokio::test]
    async fn replacing_the_owner_during_review_cannot_redirect_an_accepted_image() {
        let bus = Bus::new();
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.png");
        image::RgbaImage::from_pixel(12, 8, image::Rgba([73, 19, 201, 255]))
            .save(&source)
            .unwrap();
        let helper = directory.path().join("review.py");
        fs::write(&helper, "import json,sys,time\nfrom pathlib import Path\njson.load(sys.stdin)\nPath(__file__).with_suffix('.ready').touch()\nwhile not Path(__file__).with_suffix('.go').exists(): time.sleep(.01)\nprint('true')\n").unwrap();
        let first = Arc::new(Mutex::new(Vec::new()));
        let old = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name("org.canopa.Shell.Wallpaper")
            .unwrap()
            .serve_at(
                "/org/canopa/Shell/Wallpaper",
                Owner {
                    applied: first.clone(),
                },
            )
            .unwrap()
            .build()
            .await
            .unwrap();
        let _backend = server(&bus, &source, &helper).await;
        let client = bus.connect().await;
        let requested_source = source.clone();
        let task = tokio::spawn(async move { apply(&client, &requested_source).await });
        tokio::time::timeout(Duration::from_secs(5), async {
            while !helper.with_extension("ready").exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        old.release_name("org.canopa.Shell.Wallpaper")
            .await
            .unwrap();
        let second = Arc::new(Mutex::new(Vec::new()));
        let _new = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name("org.canopa.Shell.Wallpaper")
            .unwrap()
            .serve_at(
                "/org/canopa/Shell/Wallpaper",
                Owner {
                    applied: second.clone(),
                },
            )
            .unwrap()
            .build()
            .await
            .unwrap();
        fs::write(helper.with_extension("go"), []).unwrap();
        assert_eq!(task.await.unwrap(), 2);
        assert!(first.lock().unwrap().is_empty());
        assert!(second.lock().unwrap().is_empty());
    }
}
