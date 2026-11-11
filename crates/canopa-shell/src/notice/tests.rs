use super::*;
use std::collections::HashMap;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::OwnedValue;

#[derive(Debug, Clone, PartialEq)]
enum Call {
    Notify {
        replaces: u32,
        summary: String,
        body: String,
        actions: Vec<String>,
        urgency: Option<u8>,
        resident: bool,
        expire: i32,
    },
    Close(u32),
}

#[derive(Default)]
struct Server {
    calls: Arc<Mutex<Vec<Call>>>,
    next: u32,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    fn get_capabilities(&self) -> Vec<String> {
        vec!["actions".into(), "body".into(), "body-markup".into()]
    }

    #[allow(clippy::too_many_arguments)]
    fn notify(
        &mut self,
        _app_name: String,
        replaces_id: u32,
        _app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let urgency = hints
            .get("urgency")
            .and_then(|value| u8::try_from(value).ok());
        let resident = hints
            .get("resident")
            .and_then(|value| bool::try_from(value).ok())
            .unwrap_or(false);
        self.calls.lock().unwrap().push(Call::Notify {
            replaces: replaces_id,
            summary,
            body,
            actions,
            urgency,
            resident,
            expire: expire_timeout,
        });
        if replaces_id != 0 {
            return replaces_id;
        }
        self.next += 1;
        self.next
    }

    fn close_notification(&mut self, id: u32) {
        self.calls.lock().unwrap().push(Call::Close(id));
    }

    #[zbus(signal)]
    async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;
}

fn config_notice(message: &str) -> Notice {
    Notice {
        summary: "Shell configuration error".into(),
        body: format!("config.kdl:3:5\n{message} <b>"),
        persistent: true,
        actions: vec![
            NoticeAction::OpenFile("/home/me/.config/canopa/config.kdl".into()),
            NoticeAction::OpenSettings(Some("widgets")),
        ],
    }
}

async fn wait_for_calls(calls: &Arc<Mutex<Vec<Call>>>, count: usize) -> Vec<Call> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let current = calls.lock().unwrap().clone();
            if current.len() >= count {
                return current;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("notification server calls")
}

#[tokio::test]
async fn notices_wait_for_the_server_replace_close_and_report_actions() {
    let bus = crate::test_bus::TestBus::new();
    let address = bus.address.clone();
    let (sender, commands) = mpsc::unbounded_channel();
    let (actions, mut invoked) = iced::futures::channel::mpsc::channel(4);
    let service = tokio::spawn(run(
        move || {
            let address = address.clone();
            async move {
                zbus::connection::Builder::address(address.as_str())
                    .ok()?
                    .build()
                    .await
                    .ok()
            }
        },
        commands,
        actions,
    ));

    // The Shell can start before Notify: the notice waits for the bus name.
    sender
        .send(Command::Show(
            NoticeSlot::Configuration,
            config_notice("first"),
        ))
        .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let server = bus.connect().await;
    server
        .object_server()
        .at(
            PATH,
            Server {
                calls: calls.clone(),
                next: 0,
            },
        )
        .await
        .unwrap();
    server.request_name(SERVICE).await.unwrap();

    let first = wait_for_calls(&calls, 1).await;
    assert_eq!(
        first[0],
        Call::Notify {
            replaces: 0,
            summary: "Shell configuration error".into(),
            body: "config.kdl:3:5\nfirst &lt;b&gt;".into(),
            actions: [
                "default",
                "Open file",
                "open-file",
                "Open file",
                "open-settings",
                "Open Settings",
            ]
            .map(String::from)
            .to_vec(),
            urgency: Some(2),
            resident: true,
            expire: 0,
        }
    );

    let interface = server
        .object_server()
        .interface::<_, Server>(PATH)
        .await
        .unwrap();
    Server::action_invoked(interface.signal_emitter(), 1, "open-settings")
        .await
        .unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), invoked.next())
            .await
            .unwrap(),
        Some(NoticeAction::OpenSettings(Some("widgets")))
    );
    Server::action_invoked(interface.signal_emitter(), 1, "default")
        .await
        .unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), invoked.next())
            .await
            .unwrap(),
        Some(NoticeAction::OpenFile(
            "/home/me/.config/canopa/config.kdl".into()
        ))
    );
    // Actions for notifications the Shell did not send are ignored.
    Server::action_invoked(interface.signal_emitter(), 99, "default")
        .await
        .unwrap();

    // Repeating the same notice does not pop it up again; a new one replaces it.
    sender
        .send(Command::Show(
            NoticeSlot::Configuration,
            config_notice("first"),
        ))
        .unwrap();
    sender
        .send(Command::Show(
            NoticeSlot::Configuration,
            config_notice("second"),
        ))
        .unwrap();
    let calls_now = wait_for_calls(&calls, 2).await;
    assert!(matches!(
        &calls_now[1],
        Call::Notify { replaces: 1, body, .. } if body.contains("second")
    ));

    // A one-off session failure uses its own slot and the server's expiry.
    sender
        .send(Command::Show(
            NoticeSlot::Session,
            Notice {
                summary: "Couldn't suspend".into(),
                body: "Blocked by Firefox".into(),
                persistent: false,
                actions: Vec::new(),
            },
        ))
        .unwrap();
    let calls_now = wait_for_calls(&calls, 3).await;
    assert!(matches!(
        &calls_now[2],
        Call::Notify { replaces: 0, urgency: Some(1), resident: false, expire: -1, actions, .. }
            if actions.is_empty()
    ));

    sender
        .send(Command::Clear(NoticeSlot::Configuration))
        .unwrap();
    let calls_now = wait_for_calls(&calls, 4).await;
    assert_eq!(calls_now[3], Call::Close(1));

    // After the user dismisses a notice, the next one is shown afresh.
    Server::notification_closed(interface.signal_emitter(), 2, 2)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    sender
        .send(Command::Show(
            NoticeSlot::Session,
            Notice {
                summary: "Couldn't suspend".into(),
                body: "Blocked by Firefox".into(),
                persistent: false,
                actions: Vec::new(),
            },
        ))
        .unwrap();
    let calls_now = wait_for_calls(&calls, 5).await;
    assert!(matches!(&calls_now[4], Call::Notify { replaces: 0, .. }));

    drop(sender);
    tokio::time::timeout(Duration::from_secs(5), service)
        .await
        .unwrap()
        .unwrap();
}

#[test]
fn notice_text_is_bounded_and_actions_launch_literal_argv() {
    assert_eq!(sanitize("a\tb\nc", 10), "a b\nc");
    assert_eq!(sanitize("abcdef", 3), "abc…");
    assert_eq!(
        NoticeAction::OpenFile("/tmp/my config.kdl".into()).argv(),
        ["xdg-open", "/tmp/my config.kdl"]
    );
    assert_eq!(
        NoticeAction::OpenSettings(Some("widgets")).argv(),
        ["canopa-settings", "--page", "widgets"]
    );
    assert_eq!(NoticeAction::OpenSettings(None).argv(), ["canopa-settings"]);
}
