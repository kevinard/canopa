use super::*;
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

type PortalCalls = Arc<Mutex<Vec<(String, HashMap<String, OwnedValue>)>>>;

#[derive(Clone)]
struct Portal {
    calls: PortalCalls,
    response: Option<u32>,
    version: u32,
    closed: Arc<AtomicUsize>,
    spoof: bool,
}

#[zbus::interface(name = "org.freedesktop.portal.OpenURI")]
impl Portal {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        self.version
    }

    #[zbus(name = "OpenURI")]
    async fn open_uri(
        &self,
        parent: &str,
        uri: &str,
        options: HashMap<String, OwnedValue>,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        assert!(parent.is_empty());
        self.calls.lock().unwrap().push((uri.to_string(), options));
        // Legacy portals may choose a different handle, and may respond before
        // returning it. The real adapter must not lose this unicast signal.
        let path =
            OwnedObjectPath::try_from("/org/freedesktop/portal/desktop/request/legacy/result")
                .unwrap();
        connection
            .object_server()
            .at(path.clone(), Request(self.closed.clone()))
            .await
            .unwrap();
        if self.spoof {
            let stranger = zbus::Connection::session().await.unwrap();
            stranger
                .emit_signal(
                    header.sender().cloned(),
                    &path,
                    "org.freedesktop.portal.Request",
                    "Response",
                    &(0u32, HashMap::<String, OwnedValue>::new()),
                )
                .await
                .unwrap();
            connection
                .emit_signal(
                    header.sender().cloned(),
                    "/org/freedesktop/portal/desktop/request/wrong/path",
                    "org.freedesktop.portal.Request",
                    "Response",
                    &(0u32, HashMap::<String, OwnedValue>::new()),
                )
                .await
                .unwrap();
        }
        if let Some(response) = self.response {
            connection
                .emit_signal(
                    header.sender().cloned(),
                    &path,
                    "org.freedesktop.portal.Request",
                    "Response",
                    &(response, HashMap::<String, OwnedValue>::new()),
                )
                .await
                .unwrap();
        }
        if self.response.is_none() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(path)
    }
}

#[tokio::test]
async fn portal_opening_handles_early_response_and_preserves_uri_and_token() {
    let (portal, server, opener) = fixture(Some(0), 5, false).await;
    assert!(opener.available());
    assert_eq!(
        opener
            .open(
                NotificationLink::parse("https://example.org/a?x=1&y=2").unwrap(),
                Some("user-token".into())
            )
            .await,
        LinkOpeningResult::Opened
    );
    let calls = portal.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "https://example.org/a?x=1&y=2");
    assert_eq!(
        <&str>::try_from(&calls[0].1["activation_token"]).unwrap(),
        "user-token"
    );
    assert!(!calls[0].1.contains_key("ask"));
    let token = <&str>::try_from(&calls[0].1["handle_token"]).unwrap();
    assert!(
        token.len() >= 32
            && token
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    );
    drop(calls);
    drop(server);
}

#[derive(Clone)]
struct Request(Arc<AtomicUsize>);
#[zbus::interface(name = "org.freedesktop.portal.Request")]
impl Request {
    fn close(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

async fn fixture(
    response: Option<u32>,
    version: u32,
    spoof: bool,
) -> (Portal, zbus::Connection, PortalNotificationLinkOpener) {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let portal = Portal {
        calls: Arc::default(),
        response,
        version,
        closed: Arc::default(),
        spoof,
    };
    let name = format!(
        "org.example.CanopaLinks.p{}.n{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    );
    let server = zbus::connection::Builder::session()
        .unwrap()
        .name(name.clone())
        .unwrap()
        .serve_at("/org/freedesktop/portal/desktop", portal.clone())
        .unwrap()
        .build()
        .await
        .unwrap();
    let opener = PortalNotificationLinkOpener::connect(
        zbus::Connection::session().await.unwrap(),
        name,
        Duration::from_millis(250),
    )
    .await;
    (portal, server, opener)
}

#[tokio::test]
async fn portal_opening_distinguishes_cancellation_failure_and_unsupported_tokens() {
    for (code, expected) in [
        (1, LinkOpeningResult::Cancelled),
        (2, LinkOpeningResult::Failed),
        (17, LinkOpeningResult::Failed),
    ] {
        let (portal, _server, opener) = fixture(Some(code), 3, true).await;
        assert_eq!(
            opener
                .open(
                    NotificationLink::parse("https://example.org/").unwrap(),
                    Some("unsupported-token".into())
                )
                .await,
            expected
        );
        let calls = portal.calls.lock().unwrap();
        assert_eq!(calls.len(), 1, "never retry cancellation or failure");
        assert!(!calls[0].1.contains_key("activation_token"));
    }
}

#[tokio::test]
async fn portal_opening_timeout_closes_the_returned_request_without_retry() {
    let (portal, _server, opener) = fixture(None, 5, false).await;
    assert_eq!(
        opener
            .open(
                NotificationLink::parse("https://example.org/").unwrap(),
                None
            )
            .await,
        LinkOpeningResult::TimedOut
    );
    assert_eq!(portal.closed.load(Ordering::SeqCst), 1);
    assert_eq!(portal.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn portal_opening_shutdown_cancels_a_pending_chooser() {
    let (portal, _server, opener) = fixture(None, 5, false).await;
    let operation = opener.open(
        NotificationLink::parse("https://example.org/").unwrap(),
        None,
    );
    let task = tokio::spawn(operation);
    tokio::time::timeout(Duration::from_secs(2), async {
        while portal.calls.lock().unwrap().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    opener.cancel_pending();
    assert_eq!(task.await.unwrap(), LinkOpeningResult::Cancelled);
    assert_eq!(portal.closed.load(Ordering::SeqCst), 1);
    assert!(!opener.available());
}
