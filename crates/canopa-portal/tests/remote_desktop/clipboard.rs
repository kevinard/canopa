//! Actual private D-Bus contract and session actor admission, with controlled I/O.
use super::*;
use canopa_portal::clipboard as clip;
use futures_util::StreamExt;
use std::fs::File;
const CLIP: &str = "org.freedesktop.impl.portal.Clipboard";
const TEXT: &str = "text/plain;charset=utf-8";

struct ClipboardControl {
    events: Option<mpsc::Receiver<clip::Event>>,
    operations: Arc<Mutex<Vec<&'static str>>>,
    peers: Arc<Mutex<Vec<File>>>,
    entered: watch::Sender<bool>,
    blocked: bool,
    written: bool,
}
impl clip::Control for ClipboardControl {
    fn events(&mut self) -> Option<futures_util::stream::BoxStream<'static, clip::Event>> {
        Some(
            futures_util::stream::unfold(self.events.take()?, |mut events| async {
                events.recv().await.map(|event| (event, events))
            })
            .boxed(),
        )
    }
    fn perform<'a>(
        &'a mut self,
        command: clip::Command,
        mut cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, zbus::fdo::Result<clip::Response>> {
        Box::pin(async move {
            self.entered.send_replace(true);
            if self.blocked {
                while !*cancelled.borrow() {
                    cancelled
                        .changed()
                        .await
                        .map_err(|_| zbus::fdo::Error::Failed("Cancelled".into()))?;
                }
                return Err(zbus::fdo::Error::Failed("Cancelled".into()));
            }
            match command {
                clip::Command::Publish(_) => {
                    self.operations.lock().unwrap().push("publish");
                    Ok(clip::Response::Unit)
                }
                clip::Command::Read(_) => {
                    self.operations.lock().unwrap().push("read");
                    let (reader, writer) = rustix::pipe::pipe().unwrap();
                    File::from(writer).write_all(b"local bytes").unwrap();
                    Ok(clip::Response::Fd(reader))
                }
                clip::Command::Write(7) if !self.written => {
                    self.written = true;
                    self.operations.lock().unwrap().push("write");
                    let (reader, writer) = rustix::pipe::pipe().unwrap();
                    self.peers.lock().unwrap().push(reader.into());
                    Ok(clip::Response::Fd(writer))
                }
                clip::Command::Done(7, _) if self.written => Ok(clip::Response::Unit),
                _ => Err(zbus::fdo::Error::InvalidArgs(
                    "Invalid clipboard command".into(),
                )),
            }
        })
    }
}
struct Granted {
    inner: Box<dyn remote::Control>,
    clipboard: ClipboardControl,
}
impl remote::Control for Granted {
    fn devices(&self) -> u32 {
        self.inner.devices()
    }
    fn streams(&self) -> &[Stream] {
        self.inner.streams()
    }
    fn closed(&self) -> watch::Receiver<bool> {
        self.inner.closed()
    }
    fn clipboard_enabled(&self) -> bool {
        true
    }
    fn clipboard(&mut self) -> Option<&mut dyn clip::Control> {
        Some(&mut self.clipboard)
    }
    fn connect<'a>(
        &'a mut self,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<OwnedFd>> {
        self.inner.connect(cancelled)
    }
    fn keyboard<'a>(
        &'a mut self,
        input: remote::KeyboardInput,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, zbus::fdo::Result<()>> {
        self.inner.keyboard(input, cancelled)
    }
    fn stop(self: Box<Self>) -> BoxFuture<'static, anyhow::Result<()>> {
        Box::pin(async move {
            let Self { inner, clipboard } = *self;
            drop(clipboard);
            inner.stop().await
        })
    }
}
struct ClipboardGrant {
    control: Box<dyn remote::Control>,
    events: mpsc::Sender<clip::Event>,
    operations: Arc<Mutex<Vec<&'static str>>>,
    peers: Arc<Mutex<Vec<File>>>,
    entered: watch::Receiver<bool>,
    stops: Arc<AtomicUsize>,
    source: watch::Sender<bool>,
}
fn clipboard_grant(blocked: bool) -> ClipboardGrant {
    let (inner, source, stops, _) = grant(0, vec![], false);
    let (events, receiving) = mpsc::channel(8);
    let operations = Arc::new(Mutex::new(Vec::new()));
    let peers = Arc::new(Mutex::new(Vec::new()));
    let (entered, entering) = watch::channel(false);
    let control = Box::new(Granted {
        inner,
        clipboard: ClipboardControl {
            events: Some(receiving),
            operations: operations.clone(),
            peers: peers.clone(),
            entered,
            blocked,
            written: false,
        },
    });
    ClipboardGrant {
        control,
        events,
        operations,
        peers,
        entered: entering,
        stops,
        source,
    }
}
async fn call<R, B>(
    connection: &Connection,
    interface: &str,
    method: &str,
    body: &B,
) -> zbus::Result<R>
where
    R: for<'de> serde::Deserialize<'de> + zbus::zvariant::Type,
    B: serde::Serialize + zbus::zvariant::DynamicType,
{
    tokio::time::timeout(Duration::from_secs(3), async {
        zbus::Proxy::new(connection, BUS_NAME, OBJECT_PATH, interface)
            .await?
            .call(method, body)
            .await
    })
    .await
    .unwrap()
}
async fn pipe_bytes(mut file: File) -> Vec<u8> {
    rustix::fs::fcntl_setfl(&file, rustix::fs::OFlags::NONBLOCK).unwrap();
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let mut buffer = [0; 64];
            match file.read(&mut buffer) {
                Ok(0) => return,
                Ok(count) => bytes.extend_from_slice(&buffer[..count]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    tokio::time::sleep(Duration::from_millis(5)).await
                }
                Err(error) => panic!("{error}"),
            }
        }
    })
    .await
    .expect("clipboard pipe stayed open after the application writer closed");
    bytes
}
async fn request_clipboard(client: &Connection, name: &str) -> zbus::Result<()> {
    call(
        client,
        CLIP,
        "RequestClipboard",
        &(path(name), Options::new()),
    )
    .await
}
async fn read(client: &Connection, name: &str) -> zbus::Result<BusFd> {
    call(client, CLIP, "SelectionRead", &(path(name), TEXT)).await
}
async fn next_event(
    stream: &mut (impl futures_util::Stream<Item = zbus::Message> + Unpin),
) -> zbus::Message {
    tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn clipboard_requires_own_remote_session_and_a_requested_started_grant() {
    let mut f = Fixture::new().await;
    f.create("capture", CAPTURE).await.unwrap();
    assert!(request_clipboard(&f.client, "capture").await.is_err());
    f.create("approved", REMOTE).await.unwrap();
    let stranger = f.bus.connect().await;
    assert!(request_clipboard(&stranger, "approved").await.is_err());
    assert!(read(&f.client, "approved").await.is_err());
    request_clipboard(&f.client, "approved").await.unwrap();
    assert!(request_clipboard(&f.client, "approved").await.is_err());
    let proxy = zbus::Proxy::new(&f.client, BUS_NAME, OBJECT_PATH, CLIP)
        .await
        .unwrap();
    let foreign_proxy = zbus::Proxy::new(&stranger, BUS_NAME, OBJECT_PATH, CLIP)
        .await
        .unwrap();
    let mut signals = proxy.receive_signal("SelectionOwnerChanged").await.unwrap();
    let mut foreign = foreign_proxy
        .receive_signal("SelectionOwnerChanged")
        .await
        .unwrap();
    let mut messages = zbus::MessageStream::from(&f.client);
    let started = f.start("approved", REMOTE);
    let interaction = f.presentation().await;
    assert!(interaction.request.clipboard);
    assert!(request_clipboard(&f.client, "approved").await.is_err());
    let grant = clipboard_grant(false);
    grant
        .events
        .send(clip::Event::OwnerChanged {
            mime_types: vec![TEXT.into()],
            session_is_owner: false,
        })
        .await
        .unwrap();
    interaction
        .reply
        .send(Ok(Some(grant.control)))
        .ok()
        .unwrap();
    let (response, values) = started.await.unwrap().unwrap();
    assert_eq!(response, 0);
    assert_eq!(values["clipboard_enabled"], true.into());
    assert_eq!(values["devices"], 0_u32.into());
    let mut replied = false;
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let message = messages.next().await.unwrap().unwrap();
            if let Ok((0, values)) = message.body().deserialize::<(u32, Options)>()
                && values.get("clipboard_enabled") == Some(&true.into())
            {
                replied = true;
            }
            if message
                .header()
                .member()
                .is_some_and(|m| m.as_str() == "SelectionOwnerChanged")
            {
                assert!(
                    replied,
                    "clipboard signal preceded successful Start response"
                );
                break;
            }
        }
    })
    .await
    .unwrap();
    drop(messages); // An unfiltered observer otherwise retains FD-bearing replies.
    let signal = next_event(&mut signals).await;
    let (key, values): (OwnedObjectPath, Options) = signal.body().deserialize().unwrap();
    assert_eq!(key, path("approved"));
    assert_eq!(values["session_is_owner"], false.into());
    assert!(
        tokio::time::timeout(Duration::from_millis(60), foreign.next())
            .await
            .is_err()
    );
    assert!(read(&stranger, "approved").await.is_err());
    let fd = read(&f.client, "approved").await.unwrap();
    let mut bytes = Vec::new();
    File::from(OwnedFd::from(fd))
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes, b"local bytes");
    let fd: BusFd = call(
        &f.client,
        CLIP,
        "SelectionWrite",
        &(path("approved"), 7_u32),
    )
    .await
    .unwrap();
    File::from(OwnedFd::from(fd))
        .write_all(b"remote bytes")
        .unwrap();
    let peer = grant.peers.lock().unwrap().pop().unwrap();
    assert_eq!(pipe_bytes(peer).await, b"remote bytes");
    assert!(
        call::<BusFd, _>(
            &f.client,
            CLIP,
            "SelectionWrite",
            &(path("approved"), 7_u32)
        )
        .await
        .is_err()
    );
    assert_eq!(*grant.operations.lock().unwrap(), vec!["read", "write"]);
    assert!(
        connect(&f.client, "approved", APP).await.is_err(),
        "clipboard-only never grants EIS input"
    );
    close(
        &f.client,
        path("approved"),
        "org.freedesktop.impl.portal.Session",
    )
    .await
    .unwrap();
    retired(&f.client, path("approved")).await;
    assert!(read(&f.client, "approved").await.is_err());
}

#[tokio::test]
async fn clipboard_denial_does_not_grant_access_and_unrequested_approval_closes() {
    let mut f = Fixture::new().await;
    f.create("denied", REMOTE).await.unwrap();
    request_clipboard(&f.client, "denied").await.unwrap();
    let started = f.start("denied", REMOTE);
    let interaction = f.presentation().await;
    let (control, _source, stops, _) = grant(1, vec![], false);
    interaction.reply.send(Ok(Some(control))).ok().unwrap();
    let (_, values) = started.await.unwrap().unwrap();
    assert_eq!(values["clipboard_enabled"], false.into());
    assert!(read(&f.client, "denied").await.is_err());
    close(
        &f.client,
        path("denied"),
        "org.freedesktop.impl.portal.Session",
    )
    .await
    .unwrap();
    retired(&f.client, path("denied")).await;
    assert_eq!(stops.load(Ordering::SeqCst), 1);
    f.create("unrequested", REMOTE).await.unwrap();
    let started = f.start("unrequested", REMOTE);
    let interaction = f.presentation().await;
    assert!(!interaction.request.clipboard);
    let grant = clipboard_grant(false);
    interaction
        .reply
        .send(Ok(Some(grant.control)))
        .ok()
        .unwrap();
    assert_eq!(started.await.unwrap().unwrap().0, 2);
    retired(&f.client, path("unrequested")).await;
    assert_eq!(grant.stops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn pending_clipboard_work_cannot_prevent_owner_source_or_shutdown_cleanup() {
    for cause in ["owner", "source", "shutdown"] {
        let mut f = Fixture::new().await;
        f.create(cause, REMOTE).await.unwrap();
        request_clipboard(&f.client, cause).await.unwrap();
        let started = f.start(cause, REMOTE);
        let interaction = f.presentation().await;
        let mut grant = clipboard_grant(true);
        interaction
            .reply
            .send(Ok(Some(grant.control)))
            .ok()
            .unwrap();
        assert_eq!(started.await.unwrap().unwrap().0, 0);
        let client = f.client.clone();
        let task = tokio::spawn(async move { read(&client, cause).await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while !*grant.entered.borrow() {
                grant.entered.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        match cause {
            "owner" => f.client.clone().close().await.unwrap(),
            "source" => {
                grant.source.send_replace(true);
            }
            "shutdown" => f.portal.shutdown().await.unwrap(),
            _ => unreachable!(),
        }
        assert!(
            tokio::time::timeout(Duration::from_secs(2), task)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        retired(&f._server, path(cause)).await;
        assert_eq!(grant.stops.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn request_without_reply_is_admitted_before_the_following_start() {
    let mut f = Fixture::new().await;
    for index in 0..8 {
        let name = format!("ordered{index}");
        f.create(&name, REMOTE).await.unwrap();
        let message = zbus::Message::method_call(OBJECT_PATH, "RequestClipboard")
            .unwrap()
            .destination(BUS_NAME)
            .unwrap()
            .interface(CLIP)
            .unwrap()
            .with_flags(zbus::message::Flags::NoReplyExpected)
            .unwrap()
            .build(&(path(&name), Options::new()))
            .unwrap();
        f.client.send(&message).await.unwrap();
        let started = f.start(&name, REMOTE);
        let interaction = f.presentation().await;
        assert!(
            interaction.request.clipboard,
            "Start overtook RequestClipboard"
        );
        interaction.reply.send(Ok(None)).ok().unwrap();
        assert_eq!(started.await.unwrap().unwrap().0, 1);
        retired(&f.client, path(&name)).await;
    }
}

#[tokio::test]
async fn a_full_clipboard_queue_revokes_without_stalling_other_interfaces() {
    let mut f = Fixture::new().await;
    f.create("busy", REMOTE).await.unwrap();
    request_clipboard(&f.client, "busy").await.unwrap();
    let started = f.start("busy", REMOTE);
    let interaction = f.presentation().await;
    let mut grant = clipboard_grant(true);
    interaction
        .reply
        .send(Ok(Some(grant.control)))
        .ok()
        .unwrap();
    assert_eq!(started.await.unwrap().unwrap().0, 0);
    let client = f.client.clone();
    let blocked = tokio::spawn(async move { read(&client, "busy").await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !*grant.entered.borrow() {
            grant.entered.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    let proxy = zbus::Proxy::new(&f.client, BUS_NAME, OBJECT_PATH, CLIP)
        .await
        .unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), proxy.get_property::<u32>("version"))
            .await
            .unwrap()
            .unwrap(),
        1
    );
    for _ in 0..80 {
        let message = zbus::Message::method_call(OBJECT_PATH, "SetSelection")
            .unwrap()
            .destination(BUS_NAME)
            .unwrap()
            .interface(CLIP)
            .unwrap()
            .with_flags(zbus::message::Flags::NoReplyExpected)
            .unwrap()
            .build(&(path("busy"), Options::new()))
            .unwrap();
        f.client.send(&message).await.unwrap();
    }
    assert!(
        tokio::time::timeout(Duration::from_secs(3), blocked)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    retired(&f._server, path("busy")).await;
    assert_eq!(grant.stops.load(Ordering::SeqCst), 1);
}
