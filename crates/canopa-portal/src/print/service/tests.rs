//! Wire/controller tests use a fake presentation/delivery port. Actual Iced,
//! staging and CUPS acceptance remain separate from these ownership checks.
use super::*;
use crate::{
    OBJECT_PATH,
    print::{
        Layout, PageSetup,
        file_target::{Pinned, Reviewed},
        test_bus::{Bus, handle},
        worker::{Destination, JobOptions, fixture_document},
    },
    print_ui::Grant,
};
use std::{
    collections::VecDeque,
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    os::{fd::AsFd, unix::net::UnixStream},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::mpsc;

enum Decision {
    Approve(Box<Approval>),
    Cancel,
    Wait,
}
struct Recorded {
    rendering: Rendering,
    copies: u32,
    path: Option<Vec<u8>>,
    format: Option<Format>,
    title: String,
}
struct Fake {
    decisions: Mutex<VecDeque<Decision>>,
    reviews: mpsc::UnboundedSender<Request>,
    job_reviews: mpsc::UnboundedSender<watch::Receiver<bool>>,
    deliveries: mpsc::UnboundedSender<Recorded>,
    snapshots: AtomicUsize,
    wait_delivery: AtomicBool,
    fail_delivery: AtomicBool,
    active: Arc<AtomicUsize>,
}
struct Active(Arc<AtomicUsize>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
async fn cancellation(mut cancelled: watch::Receiver<bool>) {
    if !*cancelled.borrow() {
        let _ = cancelled.changed().await;
    }
}
impl Backend for Fake {
    fn review_jobs(
        &self,
        _: super::super::Context,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async move {
            self.active.fetch_add(1, Ordering::SeqCst);
            let _active = Active(self.active.clone());
            self.job_reviews.send(cancelled.clone()).unwrap();
            cancellation(cancelled).await;
            Ok(())
        })
    }

    fn verify_file<'a>(
        &'a self,
        pin: &'a Pinned,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<()>>> {
        Box::pin(async move {
            if *cancelled.borrow() {
                return Ok(None);
            }
            pin.verify()?;
            Ok(Some(()))
        })
    }
    fn review(
        &self,
        request: Request,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<Approval>>> {
        Box::pin(async move {
            self.reviews.send(request).unwrap();
            let decision = self
                .decisions
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Decision::Cancel);
            match decision {
                Decision::Approve(approval) => Ok(Some(*approval)),
                Decision::Cancel => Ok(None),
                Decision::Wait => {
                    cancellation(cancelled).await;
                    Ok(None)
                }
            }
        })
    }
    fn snapshot(
        &self,
        source: OwnedFd,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<Frozen>>> {
        Box::pin(async move {
            if *cancelled.borrow() {
                return Ok(None);
            }
            self.snapshots.fetch_add(1, Ordering::SeqCst);
            let mut bytes = Vec::new();
            File::from(source).read_to_end(&mut bytes)?;
            Ok(Some(fixture_document(&bytes)))
        })
    }
    fn deliver(
        &self,
        delivery: Delivery,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<()>>> {
        Box::pin(async move {
            self.active.fetch_add(1, Ordering::SeqCst);
            let _active = Active(self.active.clone());
            self.deliveries
                .send(Recorded {
                    rendering: delivery.rendering,
                    copies: delivery.approval.grant.layout.copies,
                    path: delivery
                        .approval
                        .file
                        .as_ref()
                        .map(|pin| pin.selection().path().to_vec()),
                    format: delivery.document.document.format,
                    title: delivery.title,
                })
                .unwrap();
            if self.wait_delivery.load(Ordering::SeqCst) {
                cancellation(cancelled).await;
                return Ok(None);
            }
            anyhow::ensure!(
                !self.fail_delivery.load(Ordering::SeqCst),
                "injected delivery failure"
            );
            Ok(Some(()))
        })
    }
    fn shutdown(&self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            while self.active.load(Ordering::SeqCst) != 0 {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
    }
}
struct Relay(tokio::task::JoinHandle<()>);
impl Drop for Relay {
    fn drop(&mut self) {
        self.0.abort();
    }
}
struct Fixture {
    _root: tempfile::TempDir,
    files: PathBuf,
    bus: Bus,
    print: Print,
    backend: Arc<Fake>,
    reviews: mpsc::UnboundedReceiver<Request>,
    job_reviews: mpsc::UnboundedReceiver<watch::Receiver<bool>>,
    deliveries: mpsc::UnboundedReceiver<Recorded>,
    _relay: Relay,
}
impl Fixture {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let files = root.path().join("files");
        fs::create_dir(&files).unwrap();
        let policy = Lockdown::open(
            crate::lockdown::Source::file(root.path().join("policy.sqlite3"), false).unwrap(),
        )
        .await
        .unwrap();
        let bus = Bus::new().await;
        let (review, reviews) = mpsc::unbounded_channel();
        let (delivery, deliveries) = mpsc::unbounded_channel();
        let (job_review, job_reviews) = mpsc::unbounded_channel();
        let backend = Arc::new(Fake {
            decisions: Mutex::default(),
            reviews: review,
            job_reviews: job_review,
            deliveries: delivery,
            snapshots: AtomicUsize::new(0),
            wait_delivery: AtomicBool::new(false),
            fail_delivery: AtomicBool::new(false),
            active: Arc::default(),
        });
        let print = Print::with_requests(backend.clone(), policy.clone(), Requests::default());
        bus.backend
            .object_server()
            .at(OBJECT_PATH, policy.clone())
            .await
            .unwrap();
        bus.backend
            .object_server()
            .at(OBJECT_PATH, print.clone())
            .await
            .unwrap();
        bus.backend
            .object_server()
            .at(OBJECT_PATH, PrinterJobs(print.clone()))
            .await
            .unwrap();
        let connection = bus.backend.clone();
        let relay = tokio::spawn(async move {
            policy.relay(connection).await.unwrap();
        });
        Self {
            _root: root,
            files,
            bus,
            print,
            backend,
            reviews,
            job_reviews,
            deliveries,
            _relay: Relay(relay),
        }
    }
    fn queue(&self, decision: Decision) {
        self.backend.decisions.lock().unwrap().push_back(decision);
    }
    async fn property(&self, peer: &Connection, name: &str, value: bool) {
        let proxy = zbus::Proxy::new(
            peer,
            self.bus.backend.unique_name().unwrap().as_str(),
            OBJECT_PATH,
            "org.freedesktop.DBus.Properties",
        )
        .await
        .unwrap();
        proxy
            .call::<_, _, ()>(
                "Set",
                &(crate::lockdown::INTERFACE, name, Value::from(value)),
            )
            .await
            .unwrap();
    }
    fn held(&self) -> usize {
        fs::read_dir("/proc/self/fd")
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                fs::read_link(entry.path()).is_ok_and(|path| path.starts_with(&self.files))
            })
            .count()
    }
    async fn no_pins(&self) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.held() != 0 {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap();
    }
    async fn review(&mut self) -> Request {
        tokio::time::timeout(Duration::from_secs(3), self.reviews.recv())
            .await
            .unwrap()
            .unwrap()
    }
    async fn delivery(&mut self) -> Recorded {
        tokio::time::timeout(Duration::from_secs(3), self.deliveries.recv())
            .await
            .unwrap()
            .unwrap()
    }
    async fn close(&self) {
        self.print.shutdown().await;
        self.print.0.policy.shutdown().await.unwrap();
    }
}
fn value(value: impl Into<Value<'static>>) -> OwnedValue {
    value.into().try_into_owned().unwrap()
}
fn printer(copies: u32) -> Approval {
    Approval {
        grant: Grant {
            target: Target::Printer(Destination {
                queue: "ReviewedQueue".into(),
                instance: None,
            }),
            layout: Layout {
                copies,
                ..Default::default()
            },
            page_setup: PageSetup::default(),
            device: JobOptions {
                copies,
                ..Default::default()
            },
        },
        file: None,
        authentication: None,
    }
}
fn file(path: &Path, copies: u32) -> Approval {
    let review = Reviewed::open(path).unwrap();
    let selection = review.selection().clone();
    let (send, receive) = UnixStream::pair().unwrap();
    review.send(&send).unwrap();
    drop(send);
    let pin = Pinned::receive(&receive, selection.clone()).unwrap();
    let mut approval = printer(copies);
    approval.grant.target = Target::File {
        path: selection.path().to_vec(),
        format: Format::Pdf,
        selection,
    };
    approval.grant.page_setup = PageSetup {
        name: Some("canopa-custom".into()),
        width: Some(210.0),
        height: Some(297.0),
        ..Default::default()
    };
    approval.file = Some(pin);
    approval
}
async fn proxy<'a>(peer: &'a Connection, backend: &'a Connection) -> zbus::Proxy<'a> {
    zbus::Proxy::new(
        peer,
        backend.unique_name().unwrap().as_str(),
        OBJECT_PATH,
        "org.freedesktop.impl.portal.Print",
    )
    .await
    .unwrap()
}
async fn prepare(
    peer: &Connection,
    backend: &Connection,
    client: &Connection,
    token: &str,
    settings: Options,
) -> zbus::Result<(u32, Options)> {
    proxy(peer, backend)
        .await
        .call(
            "PreparePrint",
            &(
                handle(client, token),
                "example.App",
                "",
                "wire title",
                settings,
                Options::new(),
                Options::new(),
            ),
        )
        .await
}
async fn print(
    peer: &Connection,
    backend: &Connection,
    client: &Connection,
    request: &str,
    token: Option<u32>,
) -> zbus::Result<(u32, Options)> {
    print_bytes(
        peer,
        backend,
        client,
        request,
        token,
        b"%PDF-1.7\nwire bytes",
    )
    .await
}
async fn print_bytes(
    peer: &Connection,
    backend: &Connection,
    client: &Connection,
    request: &str,
    token: Option<u32>,
    bytes: &[u8],
) -> zbus::Result<(u32, Options)> {
    let mut source = tempfile::tempfile().unwrap();
    source.write_all(bytes).unwrap();
    source.seek(SeekFrom::Start(0)).unwrap();
    let options = token
        .map(|token| Options::from([("token".into(), token.into())]))
        .unwrap_or_default();
    proxy(peer, backend)
        .await
        .call(
            "Print",
            &(
                handle(client, request),
                "example.App",
                "",
                "wire title",
                zbus::zvariant::Fd::from(source.as_fd()),
                options,
            ),
        )
        .await
}
fn token(values: &mut Options) -> u32 {
    u32::try_from(values.remove("token").unwrap()).unwrap()
}
async fn close(
    peer: &Connection,
    backend: &Connection,
    path: &OwnedObjectPath,
) -> zbus::Result<()> {
    zbus::Proxy::new(
        peer,
        backend.unique_name().unwrap().as_str(),
        path,
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap()
    .call("Close", &())
    .await
}

#[tokio::test]
async fn prepared_printer_reply_formats_one_copy_and_retains_the_approved_count() {
    let mut fixture = Fixture::new().await;
    let peer = fixture.bus.connect().await;
    fixture.queue(Decision::Approve(Box::new(printer(3))));
    let (response, mut values) = prepare(
        &peer,
        &fixture.bus.backend,
        &peer,
        "onecopy",
        Options::new(),
    )
    .await
    .unwrap();
    assert_eq!(response, 0);
    let token = token(&mut values);
    let settings = Options::try_from(values.remove("settings").unwrap()).unwrap();
    assert_eq!(
        String::try_from(settings["printer"].try_clone().unwrap()).unwrap(),
        "ReviewedQueue"
    );
    assert_eq!(
        String::try_from(settings["n-copies"].try_clone().unwrap()).unwrap(),
        "1"
    );
    assert_eq!(
        print(
            &peer,
            &fixture.bus.backend,
            &peer,
            "deliveronecopy",
            Some(token)
        )
        .await
        .unwrap()
        .0,
        0
    );
    assert_eq!(fixture.deliveries.recv().await.unwrap().copies, 3);
}

#[tokio::test]
async fn prepared_wire_reply_owns_file_pins_and_keeps_layout_provenance_until_one_use_delivery() {
    let mut fixture = Fixture::new().await;
    let peer = fixture.bus.connect().await;
    let target = fixture.files.join("chosen.pdf");
    fixture.queue(Decision::Approve(Box::new(file(&target, 3))));
    let settings = Options::from([
        ("output-uri".into(), value("file:///unreviewed.pdf")),
        ("cups-unchecked".into(), value("do not forward")),
    ]);
    let (response, mut values) = prepare(&peer, &fixture.bus.backend, &peer, "prepare", settings)
        .await
        .unwrap();
    assert_eq!(response, 0);
    let approved = token(&mut values);
    let settings = Options::try_from(values.remove("settings").unwrap()).unwrap();
    assert_eq!(
        String::try_from(settings.get("n-copies").unwrap().try_clone().unwrap()).unwrap(),
        "3"
    );
    assert_eq!(
        String::try_from(
            settings
                .get("output-file-format")
                .unwrap()
                .try_clone()
                .unwrap()
        )
        .unwrap(),
        "pdf"
    );
    assert!(!settings.contains_key("output-uri") && !settings.contains_key("cups-unchecked"));
    assert_eq!(fixture.held(), 1);
    assert!(!target.exists());
    assert!(matches!(fixture.review().await.purpose, Purpose::Prepare));
    assert_eq!(
        print(&peer, &fixture.bus.backend, &peer, "print", Some(approved))
            .await
            .unwrap()
            .0,
        0
    );
    let delivered = fixture.delivery().await;
    assert_eq!(delivered.rendering, Rendering::Prepared);
    assert_eq!(delivered.copies, 3);
    assert_eq!(
        delivered.path.unwrap(),
        target.as_os_str().as_encoded_bytes()
    );
    assert_eq!(delivered.format, Some(Format::Pdf));
    assert_eq!(delivered.title, "wire title");
    fixture.no_pins().await;
    fixture.queue(Decision::Cancel);
    assert_eq!(
        print(&peer, &fixture.bus.backend, &peer, "reuse", Some(approved))
            .await
            .unwrap()
            .0,
        1
    );
    assert!(matches!(
        fixture.review().await.purpose,
        Purpose::Direct {
            format: Some(Format::Pdf)
        }
    ));
    assert!(fixture.deliveries.try_recv().is_err());
    fixture.close().await;
}

#[tokio::test]
async fn frontend_original_clients_and_direct_impersonators_cannot_consume_each_others_tokens() {
    let mut fixture = Fixture::new().await;
    let frontend = fixture.bus.frontend().await;
    let victim = fixture.bus.connect().await;
    let stranger = fixture.bus.connect().await;
    fixture.queue(Decision::Approve(Box::new(printer(2))));
    let (_, mut values) = prepare(
        &frontend,
        &fixture.bus.backend,
        &victim,
        "prepare",
        Options::new(),
    )
    .await
    .unwrap();
    let approved = token(&mut values);
    fixture.queue(Decision::Cancel);
    assert_eq!(
        print(
            &frontend,
            &fixture.bus.backend,
            &stranger,
            "stranger",
            Some(approved)
        )
        .await
        .unwrap()
        .0,
        1
    );
    fixture.queue(Decision::Cancel);
    assert_eq!(
        print(
            &stranger,
            &fixture.bus.backend,
            &victim,
            "forged",
            Some(approved)
        )
        .await
        .unwrap()
        .0,
        1
    );
    assert_eq!(
        print(
            &frontend,
            &fixture.bus.backend,
            &victim,
            "owner",
            Some(approved)
        )
        .await
        .unwrap()
        .0,
        0
    );
    assert_eq!(fixture.delivery().await.rendering, Rendering::Prepared);
    fixture.close().await;
}

#[tokio::test]
async fn concurrent_prints_consume_a_token_once_and_the_loser_requires_fresh_consent() {
    let mut fixture = Fixture::new().await;
    let peer = fixture.bus.connect().await;
    fixture.queue(Decision::Approve(Box::new(printer(1))));
    let (_, mut values) = prepare(
        &peer,
        &fixture.bus.backend,
        &peer,
        "prepare",
        Options::new(),
    )
    .await
    .unwrap();
    let approved = token(&mut values);
    fixture.queue(Decision::Cancel);
    let (first, second) = tokio::join!(
        print(&peer, &fixture.bus.backend, &peer, "one", Some(approved)),
        print(&peer, &fixture.bus.backend, &peer, "two", Some(approved))
    );
    let mut responses = [first.unwrap().0, second.unwrap().0];
    responses.sort();
    assert_eq!(responses, [0, 1]);
    assert_eq!(fixture.delivery().await.rendering, Rendering::Prepared);
    assert!(fixture.deliveries.try_recv().is_err());
    fixture.close().await;
}

#[tokio::test]
async fn only_the_request_sender_can_close_and_original_client_loss_cancels_frontend_calls() {
    let mut fixture = Fixture::new().await;
    let peer = fixture.bus.connect().await;
    let stranger = fixture.bus.connect().await;
    fixture.queue(Decision::Wait);
    let pending = tokio::spawn({
        let peer = peer.clone();
        let backend = fixture.bus.backend.clone();
        async move { prepare(&peer, &backend, &peer, "close", Options::new()).await }
    });
    fixture.review().await;
    let path = handle(&peer, "close");
    assert!(close(&stranger, &fixture.bus.backend, &path).await.is_err());
    assert!(!pending.is_finished());
    close(&peer, &fixture.bus.backend, &path).await.unwrap();
    assert_eq!(pending.await.unwrap().unwrap().0, 1);
    assert!(close(&peer, &fixture.bus.backend, &path).await.is_err());

    let frontend = fixture.bus.frontend().await;
    let client = fixture.bus.connect().await;
    fixture.queue(Decision::Wait);
    let pending = tokio::spawn({
        let frontend = frontend.clone();
        let backend = fixture.bus.backend.clone();
        let client = client.clone();
        async move { prepare(&frontend, &backend, &client, "loss", Options::new()).await }
    });
    fixture.review().await;
    client.close().await.unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), pending)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .0,
        1
    );
    fixture.close().await;
}

#[tokio::test]
async fn printing_denial_is_enforced_before_ui_or_snapshot_and_reenable_does_not_revive_file_tokens()
 {
    let mut fixture = Fixture::new().await;
    let peer = fixture.bus.connect().await;
    fixture.property(&peer, "disable-printing", true).await;
    assert!(
        prepare(&peer, &fixture.bus.backend, &peer, "denied", Options::new())
            .await
            .is_err()
    );
    assert!(
        print(&peer, &fixture.bus.backend, &peer, "deniedprint", None)
            .await
            .is_err()
    );
    assert!(fixture.reviews.try_recv().is_err());
    assert_eq!(fixture.backend.snapshots.load(Ordering::SeqCst), 0);
    fixture.property(&peer, "disable-printing", false).await;
    fixture.queue(Decision::Approve(Box::new(file(
        &fixture.files.join("chosen.pdf"),
        1,
    ))));
    let (_, mut values) = prepare(
        &peer,
        &fixture.bus.backend,
        &peer,
        "prepare",
        Options::new(),
    )
    .await
    .unwrap();
    let approved = token(&mut values);
    fixture.property(&peer, "disable-printing", true).await;
    fixture.property(&peer, "disable-printing", false).await;
    fixture.no_pins().await;
    fixture.queue(Decision::Cancel);
    assert_eq!(
        print(&peer, &fixture.bus.backend, &peer, "stale", Some(approved))
            .await
            .unwrap()
            .0,
        1
    );
    assert!(fixture.deliveries.try_recv().is_err());
    fixture.close().await;
}

#[tokio::test]
async fn saving_denial_retires_only_file_approvals_and_disables_new_file_choices() {
    let mut fixture = Fixture::new().await;
    let peer = fixture.bus.connect().await;
    fixture.queue(Decision::Approve(Box::new(file(
        &fixture.files.join("chosen.pdf"),
        1,
    ))));
    let (_, mut values) = prepare(&peer, &fixture.bus.backend, &peer, "file", Options::new())
        .await
        .unwrap();
    let file_token = token(&mut values);
    fixture.queue(Decision::Approve(Box::new(printer(4))));
    let (_, mut values) = prepare(
        &peer,
        &fixture.bus.backend,
        &peer,
        "printer",
        Options::new(),
    )
    .await
    .unwrap();
    let printer_token = token(&mut values);
    fixture.property(&peer, "disable-save-to-disk", true).await;
    fixture.no_pins().await;
    assert_eq!(
        print(
            &peer,
            &fixture.bus.backend,
            &peer,
            "printerprint",
            Some(printer_token)
        )
        .await
        .unwrap()
        .0,
        0
    );
    assert_eq!(fixture.delivery().await.copies, 4);
    fixture.queue(Decision::Cancel);
    assert_eq!(
        print(
            &peer,
            &fixture.bus.backend,
            &peer,
            "fileprint",
            Some(file_token)
        )
        .await
        .unwrap()
        .0,
        1
    );
    fixture.review().await;
    fixture.review().await;
    assert!(fixture.review().await.prepare.formats.is_empty());
    fixture.close().await;
}

#[tokio::test]
async fn policy_changes_and_shutdown_cancel_inflight_delivery_without_a_false_success() {
    for shutdown in [false, true] {
        let mut fixture = Fixture::new().await;
        let peer = fixture.bus.connect().await;
        fixture.queue(Decision::Approve(Box::new(file(
            &fixture.files.join("chosen.pdf"),
            1,
        ))));
        let (_, mut values) = prepare(
            &peer,
            &fixture.bus.backend,
            &peer,
            "prepare",
            Options::new(),
        )
        .await
        .unwrap();
        let approved = token(&mut values);
        fixture.backend.wait_delivery.store(true, Ordering::SeqCst);
        let pending = tokio::spawn({
            let peer = peer.clone();
            let backend = fixture.bus.backend.clone();
            async move { print(&peer, &backend, &peer, "delivery", Some(approved)).await }
        });
        fixture.delivery().await;
        if shutdown {
            fixture.print.shutdown().await;
        } else {
            fixture.property(&peer, "disable-save-to-disk", true).await;
        }
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(3), pending)
                .await
                .unwrap()
                .unwrap()
                .unwrap()
                .0,
            1
        );
        fixture.no_pins().await;
        fixture.close().await;
    }
}

#[tokio::test]
async fn unreturned_approvals_release_pins_and_failed_delivery_never_reports_success() {
    let mut fixture = Fixture::new().await;
    let peer = fixture.bus.connect().await;
    fixture.queue(Decision::Approve(Box::new(file(
        &fixture.files.join("chosen.pdf"),
        1,
    ))));
    let owner = fixture.bus.owner(&peer, &peer, "example.App").await;
    let (_cancel, cancelled) = watch::channel(false);
    let request = PrepareRequest::parse(
        "example.App".into(),
        "".into(),
        "title".into(),
        Options::new(),
        Options::new(),
        Options::new(),
    )
    .unwrap();
    let preparation = fixture
        .print
        .prepare(owner, request, cancelled)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fixture.held(), 1);
    let unused = preparation.token.token();
    drop(preparation);
    fixture.no_pins().await;
    fixture.queue(Decision::Cancel);
    assert_eq!(
        print(&peer, &fixture.bus.backend, &peer, "unused", Some(unused))
            .await
            .unwrap()
            .0,
        1
    );
    fixture.queue(Decision::Approve(Box::new(printer(1))));
    let (_, mut values) = prepare(
        &peer,
        &fixture.bus.backend,
        &peer,
        "prepare",
        Options::new(),
    )
    .await
    .unwrap();
    fixture.backend.fail_delivery.store(true, Ordering::SeqCst);
    assert_eq!(
        print(
            &peer,
            &fixture.bus.backend,
            &peer,
            "failed",
            Some(token(&mut values))
        )
        .await
        .unwrap()
        .0,
        2
    );
    assert_eq!(fixture.delivery().await.rendering, Rendering::Prepared);
    fixture.close().await;
}

#[tokio::test]
async fn unknown_tokens_require_direct_review_and_the_wire_contract_has_two_distinct_outputs() {
    let mut fixture = Fixture::new().await;
    let peer = fixture.bus.connect().await;
    fixture.queue(Decision::Approve(Box::new(printer(7))));
    assert_eq!(
        print(&peer, &fixture.bus.backend, &peer, "direct", Some(u32::MAX))
            .await
            .unwrap()
            .0,
        0
    );
    let delivered = fixture.delivery().await;
    assert_eq!(delivered.rendering, Rendering::Direct);
    assert_eq!(delivered.copies, 7);
    let request = fixture.review().await;
    assert!(matches!(
        request.purpose,
        Purpose::Direct {
            format: Some(Format::Pdf)
        }
    ));
    let xml: String = zbus::Proxy::new(
        &peer,
        fixture.bus.backend.unique_name().unwrap().as_str(),
        OBJECT_PATH,
        "org.freedesktop.DBus.Introspectable",
    )
    .await
    .unwrap()
    .call("Introspect", &())
    .await
    .unwrap();
    let interface = xml
        .split("<interface name=\"org.freedesktop.impl.portal.Print\">")
        .nth(1)
        .unwrap()
        .split("</interface>")
        .next()
        .unwrap();
    assert!(interface.contains("name=\"PreparePrint\"") && interface.contains("name=\"Print\""));
    assert!(interface.contains("type=\"h\""));
    assert_eq!(interface.matches("direction=\"out\"").count(), 4);
    assert!(!interface.contains("<property"));
    fixture.close().await;
}

#[tokio::test]
async fn prepared_file_approvals_refuse_late_occupants_and_different_document_formats() {
    let mut fixture = Fixture::new().await;
    let peer = fixture.bus.connect().await;
    let target = fixture.files.join("late.pdf");
    fixture.queue(Decision::Approve(Box::new(file(&target, 1))));
    let (_, mut values) = prepare(
        &peer,
        &fixture.bus.backend,
        &peer,
        "lateprepare",
        Options::new(),
    )
    .await
    .unwrap();
    fs::write(&target, b"late occupant").unwrap();
    assert_eq!(
        print(
            &peer,
            &fixture.bus.backend,
            &peer,
            "lateprint",
            Some(token(&mut values))
        )
        .await
        .unwrap()
        .0,
        2
    );
    assert_eq!(fs::read(&target).unwrap(), b"late occupant");
    fixture.no_pins().await;
    let target = fixture.files.join("format.pdf");
    fixture.queue(Decision::Approve(Box::new(file(&target, 1))));
    let (_, mut values) = prepare(
        &peer,
        &fixture.bus.backend,
        &peer,
        "formatprepare",
        Options::new(),
    )
    .await
    .unwrap();
    assert!(
        print_bytes(
            &peer,
            &fixture.bus.backend,
            &peer,
            "formatprint",
            Some(token(&mut values)),
            b"%!PS-Adobe-3.0\nother format"
        )
        .await
        .is_err()
    );
    assert!(!target.exists());
    fixture.no_pins().await;
    assert!(fixture.deliveries.try_recv().is_err());
    fixture.close().await;
}

#[tokio::test]
async fn oversized_messages_and_wrong_setting_types_are_rejected_before_presentation() {
    let mut fixture = Fixture::new().await;
    let peer = fixture.bus.connect().await;
    assert!(
        prepare(
            &peer,
            &fixture.bus.backend,
            &peer,
            "wrongtype",
            Options::from([("n-copies".into(), 3_u32.into())])
        )
        .await
        .is_err()
    );
    let options = Options::from([(
        "ignored".into(),
        Value::from(vec![0_u8; super::super::MAX_REQUEST_BYTES as usize])
            .try_into_owned()
            .unwrap(),
    )]);
    let result: zbus::Result<(u32, Options)> = proxy(&peer, &fixture.bus.backend)
        .await
        .call(
            "PreparePrint",
            &(
                handle(&peer, "oversized"),
                "example.App",
                "",
                "title",
                Options::new(),
                Options::new(),
                options,
            ),
        )
        .await;
    assert!(result.is_err());
    assert!(fixture.reviews.try_recv().is_err());
    assert_eq!(fixture.backend.snapshots.load(Ordering::SeqCst), 0);
    fixture.close().await;
}

#[tokio::test]
async fn live_job_review_remembers_printing_denial_and_original_request_lifetime() {
    for mode in ["close", "caller", "lockdown", "coalesced", "shutdown"] {
        let mut fixture = Fixture::new().await;
        let peer = fixture.bus.connect().await;
        let path = handle(&peer, "live_jobs");
        let pending = tokio::spawn({
            let peer = peer.clone();
            let backend = fixture.bus.backend.clone();
            let path = path.clone();
            async move {
                let proxy = zbus::Proxy::new(
                    &peer,
                    backend.unique_name().unwrap().as_str(),
                    OBJECT_PATH,
                    "org.canopa.Portal.PrintJobs",
                )
                .await
                .unwrap();
                proxy
                    .call::<_, _, u32>("Review", &(path, String::new()))
                    .await
            }
        });
        let mut cancellation =
            tokio::time::timeout(Duration::from_secs(3), fixture.job_reviews.recv())
                .await
                .unwrap()
                .unwrap();
        match mode {
            "close" => {
                let request = zbus::Proxy::new(
                    &peer,
                    fixture.bus.backend.unique_name().unwrap().as_str(),
                    path.as_str(),
                    "org.freedesktop.impl.portal.Request",
                )
                .await
                .unwrap();
                request.call::<_, _, ()>("Close", &()).await.unwrap();
            }
            "caller" => {
                peer.clone().close().await.unwrap();
            }
            "lockdown" | "coalesced" => {
                fixture.property(&peer, "disable-printing", true).await;
                if mode == "coalesced" {
                    fixture.property(&peer, "disable-printing", false).await;
                }
            }
            "shutdown" => fixture.print.shutdown().await,
            _ => unreachable!(),
        }
        tokio::time::timeout(Duration::from_secs(3), async {
            while !*cancellation.borrow() {
                cancellation.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        let result = tokio::time::timeout(Duration::from_secs(3), pending)
            .await
            .unwrap()
            .unwrap();
        if mode == "caller" {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap(), 1, "{mode}");
        }
        fixture.close().await;
    }
}

#[tokio::test]
async fn disabled_printing_rejects_live_job_review_before_the_backend_can_prompt() {
    let mut fixture = Fixture::new().await;
    let peer = fixture.bus.connect().await;
    fixture.property(&peer, "disable-printing", true).await;
    let proxy = zbus::Proxy::new(
        &peer,
        fixture.bus.backend.unique_name().unwrap().as_str(),
        OBJECT_PATH,
        "org.canopa.Portal.PrintJobs",
    )
    .await
    .unwrap();
    assert!(
        proxy
            .call::<_, _, u32>("Review", &(handle(&peer, "disabled_jobs"), String::new()))
            .await
            .is_err()
    );
    assert!(fixture.job_reviews.try_recv().is_err());
    fixture.close().await;
}
